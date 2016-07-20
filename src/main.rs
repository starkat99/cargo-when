#[macro_use(crate_version)]
extern crate clap;
extern crate semver;

use std::env;
use std::io::prelude::*;
use std::ffi::OsStr;
use std::process::{exit, Command, Stdio};
use std::borrow::Cow;
use clap::{App, AppSettings, SubCommand, Arg, ArgGroup, Values, OsValues, ArgMatches};
use semver::{Version, VersionReq, Identifier, ReqParseError};

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

    /// Does a string match the rust compiler's channel
    fn matches_channel(&self, channel: &str) -> bool {
        self.channel == channel.to_lowercase()
    }

    /// Does the rust compiler version match a version requirement string
    fn matches_version(&self, version: &str) -> Result<bool, ReqParseError> {
        let versreq = try!(VersionReq::parse(version));
        Ok(versreq.matches(&self.version))
    }

    /// Do any of the channel values match?
    fn matches_any_channels<'a, 'b>(&'a self, channels: Option<Values<'b>>) -> bool {
        channels.map_or(true,
                        |mut channels| channels.any(|ch| self.matches_channel(ch)))
    }

    /// Do any of the version values match?
    fn matches_any_versions<'a, 'b>(&'a self,
                                    versions: Option<Values<'b>>)
                                    -> Result<bool, ReqParseError> {
        match versions {
            Some(vers) => {
                for v in vers {
                    if try!(self.matches_version(v)) {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            None => Ok(true),
        }
    }
}

/// A parsed environment variable requirement such as 'RUST_SRC_PATH=~/rustsrc'
struct EnvVarReq<'a> {
    name: &'a str,
    value: &'a str,
}

impl<'a> EnvVarReq<'a> {
    /// Parse and validate the environment variable requirement string
    fn parse(var: &'a str) -> Result<EnvVarReq<'a>, String> {
        let mut split = var.splitn(2, '=');

        let name = try!(split.next().ok_or("Environment variable requirement has no name"));
        if name.is_empty() {
            return Err("Invalid environment variable name".to_string());
        }
        let value = try!(split.next()
            .ok_or(format!("Invalid environment variable requirement, expecting: {}=VALUE",
                           name)));

        Ok(EnvVarReq {
            name: name,
            value: value,
        })
    }

    /// Does the requirement match an environment variable value?
    fn matches(&self) -> bool {
        // Don't try to convert OsString to unicode, just conver unicode to OsString.
        env::var_os(self.name).map_or(false, |v| v == *self.value)
    }
}

/// Do any of the environment variables exist?
fn any_env_vars_exist<'a>(vars: Option<OsValues<'a>>) -> bool {
    vars.map_or(true, |mut vars| vars.any(|v| env::var_os(v).is_some()))
}

/// Do any of the environment variable values match?
fn matches_any_env_vars<'a>(vars: Option<Values<'a>>) -> Result<bool, String> {
    match vars {
        Some(v) => {
            for var in v {
                let req = try!(EnvVarReq::parse(var));
                if req.matches() {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        None => Ok(true),
    }
}

/// Do all the command line options match?
fn options_match<'a>(sub: &'a ArgMatches<'a>) -> bool {
    // Query rustc version
    let rustc_info = RustCInfo::get_info();

    // Do all the provided match options match the current compiler and environment?
    let env_matches = matches_any_env_vars(sub.values_of("ENV-VARIABLE=VALUE"));
    if env_matches.is_err() {
        writeln!(std::io::stderr(), "{}", env_matches.unwrap_err()).ok();
        writeln!(std::io::stderr(), "{}", sub.usage()).ok();
        exit(1);
    }

    let vers_matches = rustc_info.matches_any_versions(sub.values_of("VERSION"));
    if vers_matches.is_err() {
        writeln!(std::io::stderr(), "{}", vers_matches.unwrap_err()).ok();
        writeln!(std::io::stderr(), "{}", sub.usage()).ok();
        exit(1);
    }

    rustc_info.matches_any_channels(sub.values_of("CHANNEL")) && vers_matches.unwrap() &&
    any_env_vars_exist(sub.values_of_os("ENV-VARIABLE")) && env_matches.unwrap()
}

/// Get the cargo command and arguments
fn get_cargo_command<'a>(sub: &'a ArgMatches<'a>) -> Vec<Cow<'a, OsStr>> {
    match sub.subcommand() {
        (external, Some(extm)) => {
            let mut cmd: Vec<Cow<'a, OsStr>> = vec![Cow::Owned(From::from(external))];
            if let Some(vals) = extm.values_of_os("") {
                cmd.extend(vals.map(|s| Cow::Borrowed(s)));
            }
            cmd
        }
        _ => {
            // No cargo subcommand was provided, print usage help message and exit
            writeln!(std::io::stderr(), "{}", sub.usage()).ok();
            exit(1);
        }
    }
}

fn main() {
    // CLI
    let matches = App::new("cargo when")
                    .bin_name("cargo")
                    .about(concat!("Runs other cargo commands conditionally upon rust compiler ",
                                    "version and environment."))
                    .version(crate_version!())
                    .setting(AppSettings::SubcommandRequiredElseHelp)
                    .setting(AppSettings::GlobalVersion)
                    .subcommand(SubCommand::with_name("when")
                        .usage("cargo when [OPTIONS] <CARGO SUBCOMMAND> [SUBCOMMAND OPTIONS]")
                        .about(concat!("Runs subsequent cargo command only when the specified ",
                                        "options match the current rust compiler version and ",
                                        "environment."))
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
                        .arg(Arg::with_name("ENV-VARIABLE")
                            .short("x")
                            .long("exists")
                            .group("matches")
                            .help("Matches when environment variable(s) exist")
                            .takes_value(true)
                            .min_values(1)
                            .require_delimiter(true)
                        )
                        .arg(Arg::with_name("ENV-VARIABLE=VALUE")
                            .short("e")
                            .long("equals")
                            .group("matches")
                            .help(concat!("Matches when environment variable(s) equals specified ",
                                            "value(s), e.g. RUST_SRC_PATH=~/rustsrc"))
                            .takes_value(true)
                            .min_values(1)
                            .require_delimiter(true)
                        )
                    )
                    // We don't use an alias even though args ar exact same because help is slightly
                    // different and the help won't properly show 'unless' when used.
                    .subcommand(SubCommand::with_name("unless")
                        .usage("cargo unless [OPTIONS] <CARGO SUBCOMMAND> [SUBCOMMAND OPTIONS]")
                        .about(concat!("Runs subsequent cargo command except when the specified ",
                                        "options match the current rust compiler version and ",
                                        "environment. This is the negation of 'cargo when'."))
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
                        .arg(Arg::with_name("ENV-VARIABLE")
                            .short("x")
                            .long("exists")
                            .group("matches")
                            .help("Matches when environment variable(s) exist")
                            .takes_value(true)
                            .min_values(1)
                            .require_delimiter(true)
                        )
                        .arg(Arg::with_name("ENV-VARIABLE=VALUE")
                            .short("e")
                            .long("equals")
                            .group("matches")
                            .help(concat!("Matches when environment variable(s) equals specified ",
                                            "value(s), e.g. RUST_SRC_PATH=~/rustsrc"))
                            .takes_value(true)
                            .min_values(1)
                            .require_delimiter(true)
                        )
                    ).get_matches();

    // Check conditions, gets command if matches, None if not
    let command = match matches.subcommand() {
        ("when", Some(sub)) => {
            if options_match(sub) {
                Some(get_cargo_command(sub))
            } else {
                None
            }
        }
        ("unless", Some(sub)) => {
            if !options_match(sub) {
                Some(get_cargo_command(sub))
            } else {
                None
            }
        }
        _ => None,
    };

    // If we're a match, the chained cargo command will be provided, otherwise, we do nothing
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
