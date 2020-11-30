# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org).

## [Unreleased]

## [0.4.6] - 2020-11-30

* [Exclude feature combinations by detecting dependencies of features.](https://github.com/taiki-e/cargo-hack/pull/85) See [#81](https://github.com/taiki-e/cargo-hack/pull/81) for more.

* [Fix an issue where `CARGO_HACK_CARGO_SRC=cross` did not work.](https://github.com/taiki-e/cargo-hack/pull/94)

## [0.4.5] - 2020-11-14

* Fix an issue where `cargo-hack` exits with exit code `0` if no subcommand or valid flag was passed.

* Fix an issue where `--no-default-features` flag was treated as `--exclude-no-default-features` when used together with `--each-feature` or `--feature-powerset`.

## [0.4.4] - 2020-11-13

No public API changes from 0.4.3.

* Remove version number from release binaries. URLs containing version numbers will continue to work, but are deprecated and will be removed in the next major version. See [#91](https://github.com/taiki-e/cargo-hack/pull/91) for more.

* Reduce the size of release binaries.

## [0.4.3] - 2020-11-08

No public API changes from 0.4.2.

Since this release, we have distributed compiled binary files of `cargo-hack` via GitHub release.
See [#89](https://github.com/taiki-e/cargo-hack/pull/89) for more.

## [0.4.2] - 2020-11-03

* [`cargo-hack` no longer include `--all-features` in feature combination if one or more features already excluded.](https://github.com/taiki-e/cargo-hack/pull/86)

* Diagnostic improvements.

## [0.4.1] - 2020-10-24

* [Add `--group-features` option.][82]

[82]: https://github.com/taiki-e/cargo-hack/pull/82

## [0.4.0] - 2020-10-21

* [Remove `--ignore-non-exist-features` flag.][62] Use `--ignore-unknown-features` flag instead.

* [Treat `--all-features` flag as one of feature combinations.][61] See [#42][42] for details.

* Add `--exclude-all-features` flag. ([#61][61], [#65][65]) See [#42][42] for details.

* [Add `--exclude-features` option. This is an alias of `--skip` option.][65]

* [Rename `--skip-no-default-features` flag to `--exclude-no-default-features`.][65]
  The old name can be used as an alias, but is deprecated.

* [Add `--include-features` option.][66] See [#66][66] for details.

* [Add `--include-deps-features` option.][70] See [#29][29] for details.

* [Fix an issue where using `--features` with `--each-feature` or `--feature-powerset` together would result in the same feature combination being performed multiple times.][64]

* [Fix handling of default features.][77]

* [Improve performance by avoiding reading and parsing Cargo manifest.][73]

* Diagnostic improvements.

[29]: https://github.com/taiki-e/cargo-hack/pull/29
[42]: https://github.com/taiki-e/cargo-hack/pull/42
[61]: https://github.com/taiki-e/cargo-hack/pull/61
[62]: https://github.com/taiki-e/cargo-hack/pull/62
[63]: https://github.com/taiki-e/cargo-hack/pull/63
[64]: https://github.com/taiki-e/cargo-hack/pull/64
[65]: https://github.com/taiki-e/cargo-hack/pull/65
[66]: https://github.com/taiki-e/cargo-hack/pull/66
[70]: https://github.com/taiki-e/cargo-hack/pull/70
[73]: https://github.com/taiki-e/cargo-hack/pull/73
[77]: https://github.com/taiki-e/cargo-hack/pull/77

## [0.3.14] - 2020-10-10

* [Add `--depth` option.][59] See [#59][59] for details.

[59]: https://github.com/taiki-e/cargo-hack/pull/59

## [0.3.13] - 2020-09-22

* [Print the command actually executed when error occurred.](https://github.com/taiki-e/cargo-hack/pull/55)

* [`--verbose` flag is no longer propagated to cargo.](https://github.com/taiki-e/cargo-hack/pull/55)

* [Improve compile time by removing some dependencies.](https://github.com/taiki-e/cargo-hack/pull/54)

## [0.3.12] - 2020-09-18

* [Allow only specified optional dependencies to be considered as features.](https://github.com/taiki-e/cargo-hack/pull/51)

## [0.3.11] - 2020-07-11

* [Added `--clean-per-run` flag.][49] See [#49][49] for details.

[49]: https://github.com/taiki-e/cargo-hack/pull/49

## [0.3.10] - 2020-06-20

* [Fixed an issue where some flags could not handle space-separated list correctly.][46]

[46]: https://github.com/taiki-e/cargo-hack/pull/46

## [0.3.9] - 2020-05-25

* [Fix an issue that `--skip` does not work for optional dependencies.][43]

[43]: https://github.com/taiki-e/cargo-hack/pull/43

## [0.3.8] - 2020-05-21

* [Added `--skip-no-default-features` flag.][41] See [#38][38] for details.

[38]: https://github.com/taiki-e/cargo-hack/pull/38
[41]: https://github.com/taiki-e/cargo-hack/pull/41

## [0.3.7] - 2020-05-20

* [Fixed an issue that runs with default features even if `--skip default` flag passed.][37]

[37]: https://github.com/taiki-e/cargo-hack/pull/37

## [0.3.6] - 2020-05-17

* [Fixed an issue that `--remove-dev-deps` flag does not work properly without subcommand.][36]

[36]: https://github.com/taiki-e/cargo-hack/pull/36

## [0.3.5] - 2020-04-24

* [Added `--optional-deps` flag.][34] See [#28][28] for details.

[28]: https://github.com/taiki-e/cargo-hack/pull/28
[34]: https://github.com/taiki-e/cargo-hack/pull/34

## [0.3.4] - 2020-04-23

* [cargo-hack now prints the total number of feature flag combinations and progress.][32]

[32]: https://github.com/taiki-e/cargo-hack/pull/32

## [0.3.3] - 2020-01-06

* [Added `--skip` option.][25] See [#24][24] for details.

[24]: https://github.com/taiki-e/cargo-hack/pull/24
[25]: https://github.com/taiki-e/cargo-hack/pull/25

## [0.3.2] - 2019-12-09

* [Added `--feature-powerset` flag to perform for the feature powerset.][23]

* [Reduced compile time of `cargo-hack` to less than half.][22]

[22]: https://github.com/taiki-e/cargo-hack/pull/22
[23]: https://github.com/taiki-e/cargo-hack/pull/23

## [0.3.1] - 2019-11-20

* [cargo-hack can now handle ctrl-c signal properly.][20] Previously there was an issue with interoperability with `--no-dev-deps` flag.

[20]: https://github.com/taiki-e/cargo-hack/pull/20

## [0.3.0] - 2019-11-13

* [cargo-hack now works on windows.][17]

* [Fixed an issue that when `--all`(`--workspace`) and `--package` flags are run in subcrate, the command does not apply to other crates in the workspace.][17]

* [Banned `--no-dev-deps` flag with builds that require dev-dependencies.][16]

* [cargo-hack is no longer does not generate temporary backup files.][14]

[14]: https://github.com/taiki-e/cargo-hack/pull/14
[16]: https://github.com/taiki-e/cargo-hack/pull/16
[17]: https://github.com/taiki-e/cargo-hack/pull/17

## [0.2.1] - 2019-11-03

* Removed warning from `--all`/`--workspace` flag. This is no longer "experimental".

## [0.2.0] - 2019-11-02

* [Implemented `--package` flag.][12]

* [Implemented `--exclude` flag.][12]

* [Renamed `--ignore-non-exist-features` flag to `--ignore-unknown-features`.][10]
  The old name can be used as an alias, but is deprecated.

[10]: https://github.com/taiki-e/cargo-hack/pull/10
[12]: https://github.com/taiki-e/cargo-hack/pull/12

## [0.1.1] - 2019-11-01

* Fixed some issues on Windows.

## [0.1.0] - 2019-10-30

Initial release

[Unreleased]: https://github.com/taiki-e/cargo-hack/compare/v0.4.6...HEAD
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
