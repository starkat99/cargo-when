# cargo-when

Cargo commands run conditionally upon rustc version and environment. Cargo `when` and `unless`
commands can aid in continuous integration scripts, among other uses.

## Documentation

Two cargo commands are provided, `when` and `unless`. `unless` is simply the negated condition of
`when` and has the exact same options. From the `when` command line help documentation:

```
Runs subsequent cargo command only when the specified options match the current rust compiler
version and environment.

USAGE:
    cargo when [OPTIONS] <CARGO SUBCOMMAND> [SUBCOMMAND OPTIONS]

FLAGS:
    -h, --help    Prints help information

OPTIONS:
    -c, --channel <CHANNEL>              Matches rustc release channel(s) [values: stable, beta,
                                            nightly]
    -x, --exists <ENV-VARIABLE>          Matches when environment variable(s) exist
    -e, --equals <ENV-VARIABLE=VALUE>    Matches when environment variable(s) equals specified
                                            value(s), e.g. RUST_SRC_PATH=~/rustsrc
    -v, --version <VERSION>              Matches rustc version(s) using same rules and version
                                            syntax as Cargo

To specify a set of multiple possible matches for an option, separate the values by a comma and no
spaces. At least one match option is required. If multiple match options are present, each option
specifies an additional match requirement for any of the set of possible values for that option.
```

### Examples

#### Basic Usage

If you only want to compile and test a crate on the nightly rust compiler, and use a specialy
"nightly" feature, use this command:

```bash
cargo when --channel nightly build --features nightly && cargo when --channel nightly test
```

If the rust compiler is not the nightly compiler, the cargo command will simply return a zero exit
code for success without actually running the `build` or `test` cargo command.

If, instead, you know that your crate doesn't build properly on nightly and simply wish to skip
nightly, either of the following examples will work:

```bash
cargo unless --channel nightly build
```

```bash
cargo when --channel stable,beta build
```

#### Multiple Requirements

You can provide multiple match requirements for the `when` and `unless` commands. Let's say you have
a crate that only builds on stable Rust 1.5 or higher. You could use the following command:

```bash
cargo when --channel stable --version 1.5 build
```

The version match option behaves exactly like specifying a Cargo dependency; it defaults to a caret
version requirement, meaning any stable Rust compiler version >= 1.5.0 but <= 2.0 will call
`cargo build`. Otherwise, nightly, beta, and 1.0 Rust will fail to build. You can use any version
dependency specification you would for Cargo for your Rust compiler! (Just be sure to add
double-quotes when the version strings contain special shell characters.)

In the following example, the crate worked until Rust 1.4, then is fixed again in 1.5 and up:

```bash
cargo when --version "<1.4,1.5" build --release
```

#### Environment Variables

You can use `when` and `unless` to check environment variables to determine whether a cargo command
should be run. The following example will only run the crate executable if the RUST_SRC_PATH
variable that the executable requires for its `srcpath` option is set to any value in the current
process environment:

```bash
cargo when --exists RUST_SRC_PATH run --srcpath="$RUST_SRC_PATH"
```

You can also test if the environment variable is set to specific values, for instance, this command
won't cause any errors in any other shells, but will only run the crate tests when run from a bash
shell, because someday somewhere this may prove useful to someone:

```bash
cargo when --equals SHELL=bash test
```

#### Example Travis CI Usage

The cargo `when` command works well in a continuous integration environment like
[Travis CI](https://travis-ci.org/). Here's one example scenario featuring nightly-only features and
building docs only on stable for later upload:

```yaml
sudo: false
language: rust
rust:
- stable
- 1.8.0
- beta
- nightly
before_script:
- |
  cargo install cargo-when
script:
- |
  cargo unless --channel=nightly build &&
  cargo when --channel=nightly build --features nightly &&
  cargo unless --channel=nightly test &&
  cargo when --channel=nightly test --features nightly &&
  cargo when --channel=stable doc
```

## License

This library is distributed under the terms of either of:

* MIT license ([LICENSE-MIT](LICENSE-MIT) or
[http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
[http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.

### Contributing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the
work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.