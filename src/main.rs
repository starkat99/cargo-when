#[macro_use(crate_version)]
extern crate clap;
extern crate semver;

use clap::{App, AppSettings, SubCommand, Arg, ArgGroup, Values};
use std::process::{exit, Command, Stdio};
use semver::{Version, VersionReq, Identifier};

/// Information on the rustc compiler version in this environment
struct RustCInfo {
    channel: String,
    version: Version,
}

impl RustCInfo {
    /// Obtains the rust compiler info
    fn get_info() -> RustCInfo {
        // Get RustC version from command output
        let output = Command::new("rustc")
            .arg("-V")
            .stdin(Stdio::null())
            .stderr(Stdio::inherit())
            .output()
            .expect("Failed to get rustc version");
        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse the string and get the version portion
        let verstr = output_str.split_whitespace()
            .nth(1)
            .expect("Failed to get rustc version string");
        let version = Version::parse(verstr).expect("Failed to parse rustc version");

        // Get channel from pre-release portion of version
        let channel = match version.pre.iter().next() {
            Some(ident) => {
                match ident {
                    &Identifier::AlphaNumeric(ref s) => s.clone(),
                    &Identifier::Numeric(_) => "unknown".to_string(),
                }
            }
            None => "stable".to_string(),
        };
        // Strip pre-release from version
        let version = Version {
            major: version.major,
            minor: version.minor,
            patch: version.patch,
            pre: vec![],
            build: vec![],
        };

        RustCInfo {
            channel: channel,
            version: version,
        }
    }

    fn matches_channel(&self, channel: &str) -> bool {
        self.channel == channel
    }

    fn matches_version(&self, version: &str) -> bool {
        let versreq = VersionReq::parse(version).expect("Invalid version argument");
        versreq.matches(&self.version)
    }

    fn matches_any<'a, 'b>(&'a self,
                           channels: Option<Values<'b>>,
                           versions: Option<Values<'b>>)
                           -> bool {
        channels.map_or(true,
                        |mut channels| channels.any(|ch| self.matches_channel(ch))) &&
        versions.map_or(true,
                        |mut versions| versions.any(|v| self.matches_version(v)))
    }
}

fn main() {
    // CLI
    let matches = App::new("cargo when")
                    .bin_name("cargo")
                    .about("Runs other cargo commands conditionally upon rustc version.")
                    .version(crate_version!())
                    .setting(AppSettings::SubcommandRequiredElseHelp)
                    //.setting(AppSettings::GlobalVersion)
                    .subcommand(SubCommand::with_name("when")
                        .usage("cargo when [OPTIONS] <CARGO SUBCOMMAND> [SUBCOMMAND OPTIONS]")
                        .about(concat!("Runs subsequent cargo command only when the specified ",
                                        "options match the current rustc version."))
                        .after_help(concat!("To specify a set of multiple possible matches for an ",
                                            "option, separate the values by a comma and no ",
                                            "spaces. At least one match option is required. If ",
                                            "multiple match options are present, each option ",
                                            "specifies an additional match requirement for any of ",
                                            "the set of possible values for that option."))
                        .setting(AppSettings::ArgRequiredElseHelp)
                        .setting(AppSettings::AllowExternalSubcommands)
                        .group(ArgGroup::with_name("matches")
                            .required(true)
                            .multiple(true)
                        )
                        .arg(Arg::with_name("CHANNEL")
                            .short("c")
                            .long("channel")
                            .group("matches")
                            .help("Matches rustc release channel(s)")
                            .takes_value(true)
                            .possible_values(&["stable", "beta", "nightly"])
                            .min_values(1)
                            .require_delimiter(true)
                        )
                        .arg(Arg::with_name("VERSION")
                            .short("v")
                            .long("version")
                            .group("matches")
                            .help(concat!("Matches rustc version(s) using same rules and version ",
                                            "syntax as Cargo"))
                            .takes_value(true)
                            .min_values(1)
                            .require_delimiter(true)
                        )
                    )
                    .subcommand(SubCommand::with_name("unless")
                        .usage("cargo unless [OPTIONS] <CARGO SUBCOMMAND> [SUBCOMMAND OPTIONS]")
                        .about(concat!("Runs subsequent cargo command except when the specified ",
                                        "options match the current rustc version. This is the ",
                                        "negation of 'cargo when'"))
                        .after_help(concat!("To specify a set of multiple possible matches for an ",
                                            "option, separate the values by a comma and no ",
                                            "spaces. At least one match option is required. If ",
                                            "multiple match options are present, each option ",
                                            "specifies an additional match requirement for any of ",
                                            "the set of possible values for that option."))
                        .setting(AppSettings::ArgRequiredElseHelp)
                        .setting(AppSettings::AllowExternalSubcommands)
                        .group(ArgGroup::with_name("matches")
                            .required(true)
                            .multiple(true)
                        )
                        .arg(Arg::with_name("CHANNEL")
                            .short("c")
                            .long("channel")
                            .group("matches")
                            .help("Matches rustc release channel(s)")
                            .takes_value(true)
                            .possible_values(&["stable", "beta", "nightly"])
                            .min_values(1)
                            .require_delimiter(true)
                        )
                        .arg(Arg::with_name("VERSION")
                            .short("v")
                            .long("version")
                            .group("matches")
                            .help(concat!("Matches rustc version(s) using same rules and version ",
                                            "syntax as Cargo"))
                            .takes_value(true)
                            .min_values(1)
                            .require_delimiter(true)
                        )
                    ).get_matches();

    // Query rustc version
    let rustc_info = RustCInfo::get_info();

    // Check conditions, gets command if matches, None if not
    let command = match matches.subcommand() {
        ("when", Some(sub)) => {
            // Check args for any matches
            if rustc_info.matches_any(sub.values_of("CHANNEL"), sub.values_of("VERSION")) {

                match sub.subcommand() {
                    (external, Some(extm)) => {
                        let mut cmd = vec![external];
                        if let Some(vals) = extm.values_of("") {
                            cmd.extend(vals);
                        }
                        Some(cmd)
                    }
                    _ => {
                        println!("{}", sub.usage());
                        exit(1);
                    }
                }
            } else {
                None
            }
        }
        ("unless", Some(sub)) => {
            // Check args for any matches, just negation of when
            if !rustc_info.matches_any(sub.values_of("CHANNEL"), sub.values_of("VERSION")) {
                match sub.subcommand() {
                    (external, Some(extm)) => {
                        let mut cmd = vec![external];
                        if let Some(vals) = extm.values_of("") {
                            cmd.extend(vals);
                        }
                        Some(cmd)
                    }
                    _ => {
                        println!("{}", sub.usage());
                        exit(1);
                    }
                }
            } else {
                None
            }
        }
        _ => None,
    };

    // Now run the chain cargo command if we're a match
    if let Some(args) = command {
        let status = Command::new("cargo")
            .args(&args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        match status {
            Ok(s) => {
                if !s.success() {
                    exit(s.code().unwrap_or(127));
                }
            }
            Err(_) => {
                exit(127);
            }
        }
    }
}
