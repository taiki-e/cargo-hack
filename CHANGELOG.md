# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org).

<!--
Note: In this file, do not use the hard wrap in the middle of a sentence for compatibility with GitHub comment style markdown rendering.
-->

## [Unreleased]

## [0.5.19] - 2022-09-22

- Fix "failed to parse `rust-version` field from manifest" error when workspace inheritance is used. ([#165](https://github.com/taiki-e/cargo-hack/pull/165))

## [0.5.18] - 2022-09-04

- Allow empty strings in `--features` (`-F`), `--exclude-features` (`--skip`), `--include-features`.
  Passing an empty string to them is now considered the same as not passing the flag. See [#163](https://github.com/taiki-e/cargo-hack/pull/163) for more.

- Distribute prebuilt binaries for aarch64 Windows.

## [0.5.17] - 2022-08-12

- Distribute prebuilt binaries for x86_64 FreeBSD. ([#160](https://github.com/taiki-e/cargo-hack/pull/160))

## [0.5.16] - 2022-07-30

- Fix an issue that a warning was displayed when excluding a feature that exists only in some crates in the workspace. ([#158](https://github.com/taiki-e/cargo-hack/pull/158))

## [0.5.15] - 2022-07-18

- Support namespaced features (features with `dep:` prefix). ([#154](https://github.com/taiki-e/cargo-hack/pull/154))

- Fix handling of quoted keys in `--no-dev-deps` and `--remove-dev-deps`. ([#152](https://github.com/taiki-e/cargo-hack/pull/152))

  This increases the rustc version required to build cargo-hack. (rustc 1.46+ -> 1.56+)
  The cargo/rustc version required to run cargo-hack remains unchanged. (cargo 1.26+)

  This also increases the compile time of cargo-hack. Consider [installing cargo-hack from prebuilt binaries](https://github.com/taiki-e/cargo-hack#from-prebuilt-binaries).

- Add metadata for cargo binstall.

## [0.5.14] - 2022-06-02

- Distribute prebuilt binaries for aarch64 macOS. ([#151](https://github.com/taiki-e/cargo-hack/pull/151))

## [0.5.13] - 2022-05-12

- Support short flag of `--features` (`-F`). ([#150](https://github.com/taiki-e/cargo-hack/pull/150))

## [0.5.12] - 2022-01-21

- Distribute prebuilt binaries for aarch64 Linux (gnu and musl).

## [0.5.11] - 2022-01-21

- Fix breakage on nightly-2022-01-20 or later. ([#146](https://github.com/taiki-e/cargo-hack/pull/146))

## [0.5.10] - 2022-01-05

- Fix handling of combined short flags. ([#143](https://github.com/taiki-e/cargo-hack/pull/143))

- Support omitting lower bound of `--version-range` in all cargo versions. ([#144](https://github.com/taiki-e/cargo-hack/pull/144))

## [0.5.9] - 2021-12-29

- Fix an error when using old cargo with a dependency graph containing 2021 edition crates. ([#138](https://github.com/taiki-e/cargo-hack/pull/138))

- Support omitting lower bound of `--version-range`. ([#139](https://github.com/taiki-e/cargo-hack/pull/139))

- Add `--keep-going` flag. ([#140](https://github.com/taiki-e/cargo-hack/pull/140))

- Fix an issue where `--feature-powerset` and `--each-feature` add `--all-features` as one of the combinations, even if it is already covered by another combination. ([#141](https://github.com/taiki-e/cargo-hack/pull/141))

## [0.5.8] - 2021-10-13

- Distribute statically linked binary on Windows MSVC. ([#131](https://github.com/taiki-e/cargo-hack/pull/131))

## [0.5.7] - 2021-08-09

- Fix an issue where cargo-hack cannot auto-detect whether color support is available on the terminal. ([#125](https://github.com/taiki-e/cargo-hack/pull/125))

## [0.5.6] - 2021-06-07

- You can now install cargo-hack using Homebrew tap on macOS and Linux: `brew install taiki-e/tap/cargo-hack`

- Documentation improvements.

## [0.5.5] - 2021-04-04

- Add `--clean-per-version` flag. ([#120](https://github.com/taiki-e/cargo-hack/pull/120))

## [0.5.4] - 2021-02-27

- Stop commit of `Cargo.lock`. ([#127](https://github.com/taiki-e/cargo-hack/pull/117))

  If you want to use cargo-hack with versions of dependencies at the time of release, please download the compiled binary from GitHub Releases.
  See [#117](https://github.com/taiki-e/cargo-hack/pull/117) for more.

- Support controls of colored output by `CARGO_TERM_COLOR`. ([#110](https://github.com/taiki-e/cargo-hack/pull/110))

- Do not run `rustup toolchain install` in `--version-range` if the toolchain already has installed. ([#109](https://github.com/taiki-e/cargo-hack/pull/109))

## [0.5.3] - 2021-01-05

- Documentation improvements.

- Exclude unneeded files from crates.io.

## [0.5.2] - 2020-12-09

- Automatically install target if specified when using `--version-range` option. ([#108](https://github.com/taiki-e/cargo-hack/pull/108))

## [0.5.1] - 2020-12-06

- Fix compatibility with old cargo of `--version-range` option. ([#106](https://github.com/taiki-e/cargo-hack/pull/106))

## [0.5.0] - 2020-12-06

- Remove deprecated `--skip-no-default-features` flag. ([#100](https://github.com/taiki-e/cargo-hack/pull/100))

  Use `--exclude-no-default-features` flag instead.

- Add `--version-range` option. See [#102](https://github.com/taiki-e/cargo-hack/pull/102) for more.

- Change some warnings to errors. ([#100](https://github.com/taiki-e/cargo-hack/pull/100))

- cargo-hack now handles SIGTERM the same as SIGINT (ctrl-c).

- GitHub Releases binaries containing version numbers are no longer distributed. See [#91](https://github.com/taiki-e/cargo-hack/pull/91) for more.

- Diagnostic improvements.

## [0.4.8] - 2020-12-03

- Fix an issue that feature combinations exclusion does not work properly when used with `--group-features`. ([#99](https://github.com/taiki-e/cargo-hack/pull/99))

## [0.4.7] - 2020-12-03

No public API changes from 0.4.6.

- Distribute `*.tar.gz` file for Windows via GitHub Releases. See [#98](https://github.com/taiki-e/cargo-hack/pull/98) for more.

- Distribute x86_64-unknown-linux-musl binary via GitHub Releases.

## [0.4.6] - 2020-11-30

- Exclude feature combinations by detecting dependencies of features. ([#85](https://github.com/taiki-e/cargo-hack/pull/85))

  This may significantly reduce the runtime of `--feature-powerset` on projects that have many features. See [#81](https://github.com/taiki-e/cargo-hack/pull/81) for more.

- Fix an issue where `CARGO_HACK_CARGO_SRC=cross` did not work. ([#94](https://github.com/taiki-e/cargo-hack/pull/94))

## [0.4.5] - 2020-11-14

- Fix an issue where `cargo-hack` exits with exit code `0` if no subcommand or valid flag was passed.

- Fix an issue where `--no-default-features` flag was treated as `--exclude-no-default-features` when used together with `--each-feature` or `--feature-powerset`.

## [0.4.4] - 2020-11-13

No public API changes from 0.4.3.

- Remove version number from release binaries. URLs containing version numbers will continue to work, but are deprecated and will be removed in the next major version. See [#91](https://github.com/taiki-e/cargo-hack/pull/91) for more.

- Reduce the size of release binaries.

## [0.4.3] - 2020-11-08

No public API changes from 0.4.2.

Since this release, we have distributed compiled binary files of `cargo-hack` via GitHub release.
See [#89](https://github.com/taiki-e/cargo-hack/pull/89) for more.

## [0.4.2] - 2020-11-03

- `cargo-hack` no longer include `--all-features` in feature combination if one or more features already excluded. ([#86](https://github.com/taiki-e/cargo-hack/pull/86))

- Diagnostic improvements.

## [0.4.1] - 2020-10-24

- Add `--group-features` option. ([#82](https://github.com/taiki-e/cargo-hack/pull/82))

## [0.4.0] - 2020-10-21

- Remove deprecated `--ignore-non-exist-features` flag. ([#62](https://github.com/taiki-e/cargo-hack/pull/62))

  Use `--ignore-unknown-features` flag instead.

- Treat `--all-features` flag as one of feature combinations. ([#61](https://github.com/taiki-e/cargo-hack/pull/61)) See [#42](https://github.com/taiki-e/cargo-hack/pull/42) for details.

- Add `--exclude-all-features` flag. ([#61](https://github.com/taiki-e/cargo-hack/pull/61), [#65](https://github.com/taiki-e/cargo-hack/pull/65)) See [#42](https://github.com/taiki-e/cargo-hack/pull/42) for details.

- Add `--exclude-features` option. This is an alias of `--skip` option. ([#65](https://github.com/taiki-e/cargo-hack/pull/65))

- Rename `--skip-no-default-features` flag to `--exclude-no-default-features`. ([#65](https://github.com/taiki-e/cargo-hack/pull/65))

  The old name can be used as an alias, but is deprecated.

- Add `--include-features` option. ([#66](https://github.com/taiki-e/cargo-hack/pull/66)) See [#66](https://github.com/taiki-e/cargo-hack/pull/66) for details.

- Add `--include-deps-features` option. ([#70](https://github.com/taiki-e/cargo-hack/pull/70)) See [#29](https://github.com/taiki-e/cargo-hack/pull/29) for details.

- Fix an issue where using `--features` with `--each-feature` or `--feature-powerset` together would result in the same feature combination being performed multiple times. ([#64](https://github.com/taiki-e/cargo-hack/pull/64))

- Fix handling of default features. ([#77](https://github.com/taiki-e/cargo-hack/pull/77))

- Improve performance by avoiding reading and parsing Cargo manifest. ([#73](https://github.com/taiki-e/cargo-hack/pull/73))

- Diagnostic improvements.

## [0.3.14] - 2020-10-10

- Add `--depth` option. ([#59](https://github.com/taiki-e/cargo-hack/pull/59)) See [#59](https://github.com/taiki-e/cargo-hack/pull/59) for details.

## [0.3.13] - 2020-09-22

- Print the command actually executed when error occurred. ([#55](https://github.com/taiki-e/cargo-hack/pull/55))

- `--verbose` flag is no longer propagated to cargo. ([#55](https://github.com/taiki-e/cargo-hack/pull/55))

- Improve compile time by removing some dependencies. ([#54](https://github.com/taiki-e/cargo-hack/pull/54))

## [0.3.12] - 2020-09-18

- Allow only specified optional dependencies to be considered as features. ([#51](https://github.com/taiki-e/cargo-hack/pull/51))

## [0.3.11] - 2020-07-11

- Added `--clean-per-run` flag. ([#49](https://github.com/taiki-e/cargo-hack/pull/49)) See [#49](https://github.com/taiki-e/cargo-hack/pull/49) for details.

## [0.3.10] - 2020-06-20

- Fixed an issue where some flags could not handle space-separated list correctly. ([#46](https://github.com/taiki-e/cargo-hack/pull/46))

## [0.3.9] - 2020-05-25

- Fix an issue that `--skip` does not work for optional dependencies. ([#43](https://github.com/taiki-e/cargo-hack/pull/43))

## [0.3.8] - 2020-05-21

- Added `--skip-no-default-features` flag. ([#41](https://github.com/taiki-e/cargo-hack/pull/41)) See [#38](https://github.com/taiki-e/cargo-hack/pull/38) for details.

## [0.3.7] - 2020-05-20

- Fixed an issue that runs with default features even if `--skip default` flag passed. ([#37](https://github.com/taiki-e/cargo-hack/pull/37))

## [0.3.6] - 2020-05-17

- Fixed an issue that `--remove-dev-deps` flag does not work properly without subcommand. ([#36](https://github.com/taiki-e/cargo-hack/pull/36))

## [0.3.5] - 2020-04-24

- Added `--optional-deps` flag. ([#34](https://github.com/taiki-e/cargo-hack/pull/34)) See [#28](https://github.com/taiki-e/cargo-hack/pull/28) for details.

## [0.3.4] - 2020-04-23

- cargo-hack now prints the total number of feature flag combinations and progress. ([#32](https://github.com/taiki-e/cargo-hack/pull/32))

## [0.3.3] - 2020-01-06

- Added `--skip` option. ([#25](https://github.com/taiki-e/cargo-hack/pull/25), thanks @kleimkuhler) See [#24](https://github.com/taiki-e/cargo-hack/pull/24) for details.

## [0.3.2] - 2019-12-09

- Added `--feature-powerset` flag to perform for the feature powerset. ([#23](https://github.com/taiki-e/cargo-hack/pull/23), thanks @kleimkuhler)

- Reduced compile time of `cargo-hack` to less than half. ([#22](https://github.com/taiki-e/cargo-hack/pull/22))

## [0.3.1] - 2019-11-20

- cargo-hack can now handle ctrl-c signal properly. ([#20](https://github.com/taiki-e/cargo-hack/pull/20)) Previously there was an issue with interoperability with `--no-dev-deps` flag.

## [0.3.0] - 2019-11-13

- cargo-hack now works on windows. ([#17](https://github.com/taiki-e/cargo-hack/pull/17))

- Fixed an issue that when `--all`(`--workspace`) and `--package` flags are run in subcrate, the command does not apply to other crates in the workspace. ([#17](https://github.com/taiki-e/cargo-hack/pull/17))

- Banned `--no-dev-deps` flag with builds that require dev-dependencies. ([#16](https://github.com/taiki-e/cargo-hack/pull/16))

- cargo-hack is no longer does not generate temporary backup files. ([#14](https://github.com/taiki-e/cargo-hack/pull/14))

## [0.2.1] - 2019-11-03

- Removed warning from `--all`/`--workspace` flag. This is no longer "experimental".

## [0.2.0] - 2019-11-02

- Implemented `--package` flag. ([#12](https://github.com/taiki-e/cargo-hack/pull/12))

- Implemented `--exclude` flag. ([#12](https://github.com/taiki-e/cargo-hack/pull/12))

- Renamed `--ignore-non-exist-features` flag to `--ignore-unknown-features`. ([#10](https://github.com/taiki-e/cargo-hack/pull/10))

  The old name can be used as an alias, but is deprecated.

## [0.1.1] - 2019-11-01

- Fixed some issues on Windows.

## [0.1.0] - 2019-10-30

Initial release

[Unreleased]: https://github.com/taiki-e/cargo-hack/compare/v0.5.19...HEAD
[0.5.19]: https://github.com/taiki-e/cargo-hack/compare/v0.5.18...v0.5.19
[0.5.18]: https://github.com/taiki-e/cargo-hack/compare/v0.5.17...v0.5.18
[0.5.17]: https://github.com/taiki-e/cargo-hack/compare/v0.5.16...v0.5.17
[0.5.16]: https://github.com/taiki-e/cargo-hack/compare/v0.5.15...v0.5.16
[0.5.15]: https://github.com/taiki-e/cargo-hack/compare/v0.5.14...v0.5.15
[0.5.14]: https://github.com/taiki-e/cargo-hack/compare/v0.5.13...v0.5.14
[0.5.13]: https://github.com/taiki-e/cargo-hack/compare/v0.5.12...v0.5.13
[0.5.12]: https://github.com/taiki-e/cargo-hack/compare/v0.5.11...v0.5.12
[0.5.11]: https://github.com/taiki-e/cargo-hack/compare/v0.5.10...v0.5.11
[0.5.10]: https://github.com/taiki-e/cargo-hack/compare/v0.5.9...v0.5.10
[0.5.9]: https://github.com/taiki-e/cargo-hack/compare/v0.5.8...v0.5.9
[0.5.8]: https://github.com/taiki-e/cargo-hack/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/taiki-e/cargo-hack/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/taiki-e/cargo-hack/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/taiki-e/cargo-hack/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/taiki-e/cargo-hack/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/taiki-e/cargo-hack/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/taiki-e/cargo-hack/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/taiki-e/cargo-hack/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/taiki-e/cargo-hack/compare/v0.4.8...v0.5.0
[0.4.8]: https://github.com/taiki-e/cargo-hack/compare/v0.4.7...v0.4.8
[0.4.7]: https://github.com/taiki-e/cargo-hack/compare/v0.4.6...v0.4.7
[0.4.6]: https://github.com/taiki-e/cargo-hack/compare/v0.4.5...v0.4.6
[0.4.5]: https://github.com/taiki-e/cargo-hack/compare/v0.4.4...v0.4.5
[0.4.4]: https://github.com/taiki-e/cargo-hack/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/taiki-e/cargo-hack/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/taiki-e/cargo-hack/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/taiki-e/cargo-hack/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/taiki-e/cargo-hack/compare/v0.3.14...v0.4.0
[0.3.14]: https://github.com/taiki-e/cargo-hack/compare/v0.3.13...v0.3.14
[0.3.13]: https://github.com/taiki-e/cargo-hack/compare/v0.3.12...v0.3.13
[0.3.12]: https://github.com/taiki-e/cargo-hack/compare/v0.3.11...v0.3.12
[0.3.11]: https://github.com/taiki-e/cargo-hack/compare/v0.3.10...v0.3.11
[0.3.10]: https://github.com/taiki-e/cargo-hack/compare/v0.3.9...v0.3.10
[0.3.9]: https://github.com/taiki-e/cargo-hack/compare/v0.3.8...v0.3.9
[0.3.8]: https://github.com/taiki-e/cargo-hack/compare/v0.3.7...v0.3.8
[0.3.7]: https://github.com/taiki-e/cargo-hack/compare/v0.3.6...v0.3.7
[0.3.6]: https://github.com/taiki-e/cargo-hack/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/taiki-e/cargo-hack/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/taiki-e/cargo-hack/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/taiki-e/cargo-hack/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/taiki-e/cargo-hack/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/taiki-e/cargo-hack/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/taiki-e/cargo-hack/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/taiki-e/cargo-hack/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/taiki-e/cargo-hack/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/taiki-e/cargo-hack/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/taiki-e/cargo-hack/releases/tag/v0.1.0
