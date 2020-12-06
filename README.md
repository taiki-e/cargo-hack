# cargo-hack

[![crates.io](https://img.shields.io/crates/v/cargo-hack.svg?style=flat-square&logo=rust)](https://crates.io/crates/cargo-hack)
[![license](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg?style=flat-square)](#license)
[![rustc](https://img.shields.io/badge/rustc-1.36+-blue.svg?style=flat-square)](https://www.rust-lang.org)
[![build status](https://img.shields.io/github/workflow/status/taiki-e/cargo-hack/CI/master?style=flat-square)](https://github.com/taiki-e/cargo-hack/actions?query=workflow%3ACI+branch%3Amaster)

A cargo subcommand to provide various options useful for testing and continuous integration.

## Installation

```sh
cargo install cargo-hack
```

To install cargo-hack requires rustc 1.36+.

cargo-hack is usually runnable with Cargo versions older than the Rust version required for installation (e.g., `cargo +1.31 hack check`). Currently, to run cargo-hack requires Cargo 1.26+.

## Usage

*See `cargo hack --help` for a complete list of options ([output is here](https://github.com/taiki-e/cargo-hack/blob/master/tests/long-help.txt)).*

`cargo-hack` is basically wrapper of `cargo` that propagates subcommand and most of the passed flags to `cargo`, but provides additional flags and changes the behavior of some existing flags.

* **`--each-feature`**

  Perform for each feature which includes default features and `--no-default-features` of the package.

  This is useful to check that each feature is working properly. (When used for this purpose, it is recommended to use with `--no-dev-deps` to avoid [rust-lang/cargo#4866].)

  ```sh
  cargo hack check --each-feature --no-dev-deps
  ```

* **`--feature-powerset`**

  Perform for the feature powerset which includes `--no-default-features` and
  default features of the package.

  This is useful to check that every combination of features is working
  properly. (When used for this purpose, it is recommended to use with
  `--no-dev-deps` to avoid [rust-lang/cargo#4866].)

  ```sh
  cargo hack check --feature-powerset --no-dev-deps
  ```

* **`--no-dev-deps`**

  Perform without dev-dependencies.

  This is a workaround for an issue that dev-dependencies leaking into normal build ([rust-lang/cargo#4866]).

  Also, this can be used as a workaround for an issue that `cargo` does not allow publishing a package with cyclic dev-dependencies. ([rust-lang/cargo#4242])

  ```sh
  cargo hack publish --no-dev-deps --dry-run --allow-dirty
  ```

  Note: Currently, using `--no-dev-deps` flag removes dev-dependencies from real manifest while cargo-hack is running and restores it when finished. See [rust-lang/cargo#4242] for why this is necessary.
  Also, this behavior may change in the future on some subcommands. See also [#15].

* **`--remove-dev-deps`**

  Equivalent to `--no-dev-deps` except for does not restore the original `Cargo.toml` after execution.

  This is useful to know what Cargo.toml that cargo-hack is actually using with `--no-dev-deps`.

  *This flag also works without subcommands.*

* **`--ignore-private`**

  Skip to perform on `publish = false` packages.

* **`--ignore-unknown-features`**

  Skip passing `--features` to `cargo` if that feature does not exist.

* **`--clean-per-run`**

  Remove artifacts for that package before running the command.

  This also works as a workaround for [rust-lang/rust-clippy#4612].

* **`--version-range`**

  Perform commands on a specified (inclusive) range of Rust versions.

  ```console
  $ cargo hack check --version-range 1.46..1.47
  info: running `cargo +1.46 check` on cargo-hack (1/2)
  ...
  info: running `cargo +1.47 check` on cargo-hack (2/2)
  ...
  ```

  If the given range is unclosed, the latest stable compiler is treated as the upper bound.

  This might be useful for catching issues like [BurntSushi/termcolor#35], [rust-lang/regex#685], [rust-lang/rust-clippy#6324].

  [BurntSushi/termcolor#35]: https://github.com/BurntSushi/termcolor/issues/35
  [rust-lang/regex#685]: https://github.com/rust-lang/regex/issues/685
  [rust-lang/rust-clippy#6324]: https://github.com/rust-lang/rust-clippy/issues/6324.

* **`--version-step`**

  Specify the version interval of `--version-range`.

The following flags can be used with `--each-feature` and `--feature-powerset`.

* **`--optional-deps`**

  Use optional dependencies as features.

* **`--exclude-features`**, **`--skip`**

  Space-separated list of features to exclude.

* **`--depth`**

  Specify a max number of simultaneous feature flags of `--feature-powerset`.

  If the number is set to 1, `--feature-powerset` is equivalent to `--each-feature`.

* **`--group-features`**

  Space-separated list of features to group.

  To specify multiple groups, use this option multiple times: `--group-features a,b --group-features c,d`

`cargo-hack` changes the behavior of the following existing flags.

* **`--features`**, **`--no-default-features`**

  Unlike `cargo` ([rust-lang/cargo#3620], [rust-lang/cargo#4106], [rust-lang/cargo#4463], [rust-lang/cargo#4753], [rust-lang/cargo#5015], [rust-lang/cargo#5364], [rust-lang/cargo#6195]), it can also be applied to sub-crates.

* **`--all`**, **`--workspace`**

  Perform command for all packages in the workspace.

  Unlike cargo, it does not compile all members at once.

  For example, running `cargo hack check --all` in a workspace with members `foo` and `bar` behaves almost the same as the following script:

  ```sh
  # If you use cargo-hack, you don't need to maintain this list manually.
  members=("foo" "bar")

  for member in "${members[@]}"; do
      cargo check --manifest-path "${member}/Cargo.toml"
  done
  ```

  *Workspace members will be performed according to the order of the 'packages' fields of [`cargo metadata`][cargo-metadata].*

[#15]: https://github.com/taiki-e/cargo-hack/issues/15
[rust-lang/cargo#3620]: https://github.com/rust-lang/cargo/issues/3620
[rust-lang/cargo#4106]: https://github.com/rust-lang/cargo/issues/4106
[rust-lang/cargo#4242]: https://github.com/rust-lang/cargo/issues/4242
[rust-lang/cargo#4463]: https://github.com/rust-lang/cargo/issues/4463
[rust-lang/cargo#4753]: https://github.com/rust-lang/cargo/issues/4753
[rust-lang/cargo#4866]: https://github.com/rust-lang/cargo/issues/4866
[rust-lang/cargo#5015]: https://github.com/rust-lang/cargo/issues/4463
[rust-lang/cargo#5364]: https://github.com/rust-lang/cargo/issues/5364
[rust-lang/cargo#6195]: https://github.com/rust-lang/cargo/issues/6195
[rust-lang/rust-clippy#4612]: https://github.com/rust-lang/cargo/issues/4612
[cargo-metadata]: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
