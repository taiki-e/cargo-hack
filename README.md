# cargo-hack

[![crates.io](https://img.shields.io/crates/v/cargo-hack?style=flat-square&logo=rust)](https://crates.io/crates/cargo-hack)
[![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue?style=flat-square)](#license)
[![rustc](https://img.shields.io/badge/rustc-1.46+-blue?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![build status](https://img.shields.io/github/workflow/status/taiki-e/cargo-hack/CI/main?style=flat-square&logo=github)](https://github.com/taiki-e/cargo-hack/actions)

Cargo subcommand to provide various options useful for testing and continuous
integration.

- [Usage](#usage)
- [Installation](#installation)
- [License](#license)

## Usage

<details>
<summary>Click to show a complete list of options</summary>

<!-- readme-long-help:start -->
```console
$ cargo hack --help
cargo-hack
Cargo subcommand to provide various options useful for testing and continuous integration.

USAGE:
    cargo hack [OPTIONS] [SUBCOMMAND]

Use -h for short descriptions and --help for more details.

OPTIONS:
    -p, --package <SPEC>...
            Package(s) to check.

        --all
            Alias for --workspace.

        --workspace
            Perform command for all packages in the workspace.

        --exclude <SPEC>...
            Exclude packages from the check.

            This flag can only be used together with --workspace

        --manifest-path <PATH>
            Path to Cargo.toml.

        --features <FEATURES>...
            Space-separated list of features to activate.

        --each-feature
            Perform for each feature of the package.

            This also includes runs with just --no-default-features flag, --all-features flag, and
            default features.

        --feature-powerset
            Perform for the feature powerset of the package.

            This also includes runs with just --no-default-features flag, --all-features flag, and
            default features.

        --optional-deps [DEPS]...
            Use optional dependencies as features.

            If DEPS are not specified, all optional dependencies are considered as features.

            This flag can only be used together with either --each-feature flag or
            --feature-powerset flag.

        --skip <FEATURES>...
            Alias for --exclude-features.

        --exclude-features <FEATURES>...
            Space-separated list of features to exclude.

            To exclude run of default feature, using value `--exclude-features default`.

            To exclude run of just --no-default-features flag, using --exclude-no-default-features
            flag.

            To exclude run of just --all-features flag, using --exclude-all-features flag.

            This flag can only be used together with either --each-feature flag or
            --feature-powerset flag.

        --exclude-no-default-features
            Exclude run of just --no-default-features flag.

            This flag can only be used together with either --each-feature flag or
            --feature-powerset flag.

        --exclude-all-features
            Exclude run of just --all-features flag.

            This flag can only be used together with either --each-feature flag or
            --feature-powerset flag.

        --depth <NUM>
            Specify a max number of simultaneous feature flags of --feature-powerset.

            If NUM is set to 1, --feature-powerset is equivalent to --each-feature.

            This flag can only be used together with --feature-powerset flag.

        --group-features <FEATURES>...
            Space-separated list of features to group.

            To specify multiple groups, use this option multiple times: `--group-features a,b
            --group-features c,d`

            This flag can only be used together with --feature-powerset flag.

        --include-features <FEATURES>...
            Include only the specified features in the feature combinations instead of package
            features.

            This flag can only be used together with either --each-feature flag or
            --feature-powerset flag.

        --no-dev-deps
            Perform without dev-dependencies.

            Note that this flag removes dev-dependencies from real `Cargo.toml` while cargo-hack is
            running and restores it when finished.

        --remove-dev-deps
            Equivalent to --no-dev-deps flag except for does not restore the original `Cargo.toml`
            after performed.

        --ignore-private
            Skip to perform on `publish = false` packages.

        --ignore-unknown-features
            Skip passing --features flag to `cargo` if that feature does not exist in the package.

            This flag can only be used together with either --features or --include-features.

        --version-range <START>..[END]
            Perform commands on a specified (inclusive) range of Rust versions.

            If the given range is unclosed, the latest stable compiler is treated as the upper
            bound.

            Note that ranges are always inclusive ranges.

        --version-step <NUM>
            Specify the version interval of --version-range (default to `1`).

            This flag can only be used together with --version-range flag.

        --clean-per-run
            Remove artifacts for that package before running the command.

            If used this flag with --workspace, --each-feature, or --feature-powerset, artifacts
            will be removed before each run.

            Note that dependencies artifacts will be preserved.

        --clean-per-version
            Remove artifacts per Rust version.

            Note that dependencies artifacts will also be removed.

            This flag can only be used together with --version-range flag.

    -v, --verbose
            Use verbose output.

        --color <WHEN>
            Coloring: auto, always, never.

            This flag will be propagated to cargo.

    -h, --help
            Prints help information.

    -V, --version
            Prints version information.

Some common cargo commands are (see all commands with --list):
    build       Compile the current package
    check       Analyze the current package and report errors, but don't build object files
    run         Run a binary or example of the local package
    test        Run the tests
```
<!-- readme-long-help:end -->

</details>

`cargo-hack` is basically wrapper of `cargo` that propagates subcommand and most
of the passed flags to `cargo`, but provides additional flags and changes the
behavior of some existing flags.

### --each-feature

Perform for each feature which includes default features and
`--no-default-features` of the package.

This is useful to check that each feature is working properly. (When used for
this purpose, it is recommended to use with `--no-dev-deps` to avoid
[cargo#4866].)

```sh
cargo hack check --each-feature --no-dev-deps
```

### --feature-powerset

Perform for the feature powerset which includes `--no-default-features` and
default features of the package.

This is useful to check that every combination of features is working
properly. (When used for this purpose, it is recommended to use with
`--no-dev-deps` to avoid [cargo#4866].)

```sh
cargo hack check --feature-powerset --no-dev-deps
```

### --no-dev-deps

Perform without dev-dependencies.

This is a workaround for an issue that dev-dependencies leaking into normal
build ([cargo#4866]).

Also, this can be used as a workaround for an issue that `cargo` does not
allow publishing a package with cyclic dev-dependencies. ([cargo#4242])

```sh
cargo hack publish --no-dev-deps --dry-run --allow-dirty
```

NOTE: Currently, using `--no-dev-deps` flag removes dev-dependencies from
real manifest while cargo-hack is running and restores it when finished.
See [cargo#4242] for why this is necessary.
Also, this behavior may change in the future on some subcommands. See also
[#15].

### --remove-dev-deps

Equivalent to `--no-dev-deps` except for does not restore the original
`Cargo.toml` after execution.

This is useful to know what Cargo.toml that cargo-hack is actually using
with `--no-dev-deps`.

*This flag also works without subcommands.*

### --ignore-private

Skip to perform on `publish = false` packages.

### --ignore-unknown-features

Skip passing `--features` to `cargo` if that feature does not exist.

### --version-range

Perform commands on a specified (inclusive) range of Rust versions.

```console
$ cargo hack check --version-range 1.46..1.47
info: running `cargo +1.46 check` on cargo-hack (1/2)
...
info: running `cargo +1.47 check` on cargo-hack (2/2)
...
```

If the given range is unclosed, the latest stable compiler is treated as the
upper bound.

This might be useful for catching issues like [termcolor#35], [regex#685],
[rust-clippy#6324].

### --version-step

Specify the version interval of `--version-range`.

### --clean-per-run

Remove artifacts for that package before running the command.

This also works as a workaround for [rust-clippy#4612].

### Options for adjusting the behavior of --each-feature and --feature-powerset

The following flags can be used with `--each-feature` and `--feature-powerset`.

#### --optional-deps

Use optional dependencies as features.

#### --exclude-features, --skip

Space-separated list of features to exclude.

#### --depth

Specify a max number of simultaneous feature flags of `--feature-powerset`.

If the number is set to 1, `--feature-powerset` is equivalent to
`--each-feature`.

#### --group-features

Space-separated list of features to group.

To specify multiple groups, use this option multiple times:
`--group-features a,b --group-features c,d`

### Improvement of the behavior of existing cargo flags

`cargo-hack` changes the behavior of the following existing flags.

#### --features, --no-default-features

Unlike `cargo` ([cargo#3620], [cargo#4106], [cargo#4463], [cargo#4753],
[cargo#5015], [cargo#5364], [cargo#6195]), it can also be applied to
sub-crates.

#### --all, --workspace

Perform command for all packages in the workspace.

Unlike cargo, it does not compile all members at once.

For example, running `cargo hack check --all` in a workspace with members
`foo` and `bar` behaves almost the same as the following script:

```sh
# If you use cargo-hack, you don't need to maintain this list manually.
members=("foo" "bar")

for member in "${members[@]}"; do
    cargo check --manifest-path "${member}/Cargo.toml"
done
```

*Workspace members will be performed according to the order of the 'packages'
fields of [`cargo metadata`][cargo-metadata].*

## Installation

### From source

```sh
cargo install cargo-hack
```

*Compiler support: requires rustc 1.46+*

cargo-hack is usually runnable with Cargo versions older than the Rust version
required for installation (e.g., `cargo +1.31 hack check`). Currently, to run
cargo-hack requires Cargo 1.26+.

### From prebuilt binaries

You can download prebuilt binaries from the [Release page](https://github.com/taiki-e/cargo-hack/releases).
Prebuilt binaries are available for macOS, Linux (gnu and musl), and Windows (static executable).

### Via Homebrew

You can install cargo-hack using [Homebrew tap on macOS and Linux](https://github.com/taiki-e/homebrew-tap/blob/main/Formula/cargo-hack.rb):

```sh
brew install taiki-e/tap/cargo-hack
```

### Via AUR (ArchLinux)

You can install [cargo-hack from AUR](https://aur.archlinux.org/packages/cargo-hack):

```sh
paru -S cargo-hack
```

NOTE: AUR package is maintained by community, not maintainer of cargo-hack.

[#15]: https://github.com/taiki-e/cargo-hack/issues/15
[termcolor#35]: https://github.com/BurntSushi/termcolor/issues/35
[cargo#3620]: https://github.com/rust-lang/cargo/issues/3620
[cargo#4106]: https://github.com/rust-lang/cargo/issues/4106
[cargo#4242]: https://github.com/rust-lang/cargo/issues/4242
[cargo#4463]: https://github.com/rust-lang/cargo/issues/4463
[cargo#4753]: https://github.com/rust-lang/cargo/issues/4753
[cargo#4866]: https://github.com/rust-lang/cargo/issues/4866
[cargo#5015]: https://github.com/rust-lang/cargo/issues/4463
[cargo#5364]: https://github.com/rust-lang/cargo/issues/5364
[cargo#6195]: https://github.com/rust-lang/cargo/issues/6195
[regex#685]: https://github.com/rust-lang/regex/issues/685
[rust-clippy#4612]: https://github.com/rust-lang/rust-clippy/issues/4612
[rust-clippy#6324]: https://github.com/rust-lang/rust-clippy/issues/6324
[cargo-metadata]: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
