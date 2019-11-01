# cargo-hack

[![crates-badge]][crates-url]
[![license-badge]][license]
[![rustc-badge]][rustc-url]

[crates-badge]: https://img.shields.io/crates/v/cargo-hack.svg
[crates-url]: https://crates.io/crates/cargo-hack
[license-badge]: https://img.shields.io/crates/l/cargo-hack.svg
[license]: #license
[rustc-badge]: https://img.shields.io/badge/rustc-1.36+-lightgray.svg
[rustc-url]: https://blog.rust-lang.org/2019/07/04/Rust-1.36.0.html

A tool to work around some limitations on cargo.

Cargo is a great tool but has some limitations.
This tool provides additional flags to avoid some of these limitations.

## Installation

```sh
cargo install cargo-hack
```

To install the current cargo-hack requires Rust 1.36 or later.

**Note: cargo-hack is currently only tested on Linux and macOS. It may not work well on other platforms.**

## Usage

`cargo-hack` is basically wrapper of `cargo` that propagates subcommand and most of the passed flags to `cargo`, but provides additional flags and changes the behavior of some existing flags.

* **`--each-feature`**

  Perform for each feature *which includes `--no-default-features` and default features* of the package.

  This is primarily intended to be used to verify that each feature is working properly. (*When used for this purpose, it is recommended to use with `--no-dev-deps`.*)

  ```sh
  cargo hack check --each-feature --no-dev-deps
  ```

  This is a workaround for an issue that cargo does not support for `--features` and `--no-default-features` flags for sub crates ([rust-lang/cargo#3620], [rust-lang/cargo#4106], [rust-lang/cargo#4463], [rust-lang/cargo#4753], [rust-lang/cargo#5015], [rust-lang/cargo#5364], [rust-lang/cargo#6195]).

* **`--no-dev-deps`**

  Perform without dev-dependencies.

  This is a workaround for an issue that dev-dependencies leaking into normal build ([rust-lang/cargo#4866]).

* **`--remove-dev-deps`**

  Equivalent to `--no-dev-deps` except for does not restore the original `Cargo.toml` after execution.

  This is useful to know what Cargo.toml that cargo-hack is actually using with `--no-dev-deps`.

  Also, this can be used as a workaround for an issue that `cargo` does not allow publishing a package with cyclic dev-dependencies. ([rust-lang/cargo#4242])

  ```sh
  # This flag also works without subcommands.
  cargo hack --remove-dev-deps
  cargo publish --dry-run --allow-dirty
  # Equivalent to `cargo hack publish --no-dev-deps --dry-run --allow-dirty`
  ```

* **`--ignore-private`**

  Skip to perform on `publish = false` packages.

* **`--ignore-unknown-features`**

  Skip passing `--features` to `cargo` if that feature does not exist.

  This is a workaround for an issue that `cargo` does not support for `--features` with workspace ([rust-lang/cargo#3620], [rust-lang/cargo#4106], [rust-lang/cargo#4463], [rust-lang/cargo#4753], [rust-lang/cargo#5015], [rust-lang/cargo#5364], [rust-lang/cargo#6195]).

  This feature was formerly called `--ignore-unknown-features`, but has been renamed. The old name can be used as an alias, but is deprecated.

`cargo-hack` changes the behavior of the following existing flags.

* **`--all`**, **`--workspace`**

  Perform command for all packages in the workspace.

  For example, running `cargo hack check --all` in a workspace with members `foo` and `bar` behaves almost the same as the following shell script:

  ```sh
  members=("foo" "bar")

  for member in ${members[@]}; do
    cd ${member}
    cargo check
    cd -
  done
  ```

  **Note that there is currently no guarantee in which order workspace members will be performed.** (This means that `cargo hack publish --all --ignore-private` does not necessarily function as you intended.)

* **`--features`**, **`--no-default-features`**

  Unlike `cargo` ([rust-lang/cargo#3620], [rust-lang/cargo#4106], [rust-lang/cargo#4463], [rust-lang/cargo#4753], [rust-lang/cargo#5015], [rust-lang/cargo#5364], [rust-lang/cargo#6195]), it can also be applied to sub-crate.

* **`-p`**, **`--package`**

  *Currently this flag is ignored.*

* **`--exclude`**

  *Currently this flag is ignored.*

[rust-lang/cargo#3620]: https://github.com/rust-lang/cargo/issues/3620
[rust-lang/cargo#4106]: https://github.com/rust-lang/cargo/issues/4106
[rust-lang/cargo#4463]: https://github.com/rust-lang/cargo/issues/4463
[rust-lang/cargo#4753]: https://github.com/rust-lang/cargo/issues/4753
[rust-lang/cargo#4866]: https://github.com/rust-lang/cargo/issues/4866
[rust-lang/cargo#5015]: https://github.com/rust-lang/cargo/issues/4463
[rust-lang/cargo#5364]: https://github.com/rust-lang/cargo/issues/5364
[rust-lang/cargo#6195]: https://github.com/rust-lang/cargo/issues/6195
[rust-lang/cargo#4242]: https://github.com/rust-lang/cargo/issues/4242

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
