#![warn(rust_2018_idioms, single_use_lifetimes)]

mod auxiliary;

use auxiliary::{cargo_hack, CommandExt, SEPARATOR};

#[test]
fn multi_arg() {
    // --package, -p, --exclude, --features, --exclude-features, and --verbose are allowed.

    for flag in &[
        "--examples",
        "--workspace",
        "--all",
        "--each-feature",
        "--feature-powerset",
        "--no-dev-deps",
        "--remove-dev-deps",
        "--ignore-private",
        "--ignore-unknown-features",
        "--optional-deps",
    ] {
        cargo_hack(["check", flag, flag])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "The argument '{}' was provided more than once, but cannot be used multiple times",
                flag
            ));
    }

    for (flag, msg) in
        &[("--manifest-path", "--manifest-path <PATH>"), ("--color", "--color <WHEN>")]
    {
        cargo_hack(["check", flag, "auto", flag, "auto"])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "The argument '{}' was provided more than once, but cannot be used multiple times",
                msg
            ));
    }
}

#[test]
fn removed_flags() {
    for (flag, alt) in &[("--ignore-non-exist-features", "--ignore-unknown-features")] {
        cargo_hack(["check", flag])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!("{} was removed, use {} instead", flag, alt));
    }
}

#[test]
fn real_manifest() {
    cargo_hack(["check"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             running `cargo check` on member2
             running `cargo check` on member3",
        )
        .assert_stderr_contains("running `cargo check` on real");

    cargo_hack(["check", "--workspace"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1 (1/4)
             running `cargo check` on member2 (2/4)
             running `cargo check` on member3 (3/4)
             running `cargo check` on real (4/4)",
        );
}

#[test]
fn virtual_manifest() {
    cargo_hack(["check"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1 (1/3)
             running `cargo check` on member2 (2/3)",
        );

    cargo_hack(["check", "--all"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1 (1/3)
             running `cargo check` on member2 (2/3)",
        );
}

#[test]
fn real_all_in_subcrate() {
    cargo_hack(["check"])
        .test_dir("tests/fixtures/real/member2")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member2")
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             running `cargo check` on member3
             running `cargo check` on real",
        );

    cargo_hack(["check", "--all"])
        .test_dir("tests/fixtures/real/member2")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2
             running `cargo check` on member3
             running `cargo check` on real",
        );
}

#[test]
fn virtual_all_in_subcrate() {
    cargo_hack(["check"])
        .test_dir("tests/fixtures/virtual/member1")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2");

    cargo_hack(["check", "--all"])
        .test_dir("tests/fixtures/virtual/member1")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2",
        );
}

#[test]
fn real_ignore_private() {
    cargo_hack(["check", "--ignore-private"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             skipped running on private crate member1
             running `cargo check` on member2
             skipped running on private crate member2
             running `cargo check` on real",
        )
        .assert_stderr_contains("skipped running on private crate real");

    cargo_hack(["check", "--all", "--ignore-private"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             skipped running on private crate member2
             running `cargo check` on member3
             skipped running on private crate real",
        )
        .assert_stderr_not_contains(
            "skipped running on private crate member1
             running `cargo check` on member2
             skipped running on private crate member3
             running `cargo check` on real",
        );
}

#[test]
fn virtual_ignore_private() {
    cargo_hack(["check", "--ignore-private"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             skipped running on private crate member2",
        )
        .assert_stderr_not_contains(
            "skipped running on private crate member1
             running `cargo check` on member2",
        );

    cargo_hack(["check", "--all", "--ignore-private"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             skipped running on private crate member2",
        )
        .assert_stderr_not_contains(
            "running `cargo check` on member2
             skipped running on private crate member1",
        );
}

#[test]
fn package() {
    cargo_hack(["check", "--package", "member1"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on member1")
        .assert_stderr_not_contains("running `cargo check` on member2");
}

#[test]
fn package_no_packages() {
    cargo_hack(["check", "--package", "foo"])
        .test_dir("tests/fixtures/virtual")
        .assert_failure()
        .assert_stderr_contains("package ID specification `foo` matched no packages");
}

#[test]
fn exclude() {
    cargo_hack(["check", "--all", "--exclude", "member1"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on member1")
        .assert_stderr_contains("running `cargo check` on member2");

    // not_found is warning
    cargo_hack(["check", "--all", "--exclude", "foo"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "excluded package(s) foo not found in workspace
             running `cargo check` on member1
             running `cargo check` on member2",
        );
}

#[test]
fn exclude_failure() {
    // not with --workspace
    cargo_hack(["check", "--exclude", "member1"])
        .test_dir("tests/fixtures/virtual")
        .assert_failure()
        .assert_stderr_contains("--exclude can only be used together with --workspace");
}

#[test]
fn no_dev_deps() {
    cargo_hack(["check", "--no-dev-deps"])
        .test_dir("tests/fixtures/no_dev_deps")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on no_dev_deps
             --no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is \
             running and restores it when finished",
        );

    // with --all
    cargo_hack(["check", "--no-dev-deps", "--all"])
        .test_dir("tests/fixtures/no_dev_deps")
        .assert_success()
        .assert_stderr_contains(
            "--no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is running and restores it when finished",
        );
}

#[test]
fn no_dev_deps_failure() {
    // with --remove-dev-deps
    cargo_hack(["check", "--no-dev-deps", "--remove-dev-deps"])
        .test_dir("tests/fixtures/no_dev_deps")
        .assert_failure()
        .assert_stderr_contains("--no-dev-deps may not be used together with --remove-dev-deps");

    // with options requires dev-deps
    for flag in
        &["--example", "--examples", "--test", "--tests", "--bench", "--benches", "--all-targets"]
    {
        cargo_hack(["check", "--no-dev-deps", flag])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--no-dev-deps may not be used together with {}",
                flag
            ));
    }

    // with subcommands requires dev-deps
    for subcommand in &["test", "bench"] {
        cargo_hack([subcommand, "--no-dev-deps"])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--no-dev-deps may not be used together with {} subcommand",
                subcommand
            ));
    }
}

#[test]
fn remove_dev_deps_failure() {
    // with options requires dev-deps
    for flag in
        &["--example", "--examples", "--test", "--tests", "--bench", "--benches", "--all-targets"]
    {
        cargo_hack(["check", "--remove-dev-deps", flag])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--remove-dev-deps may not be used together with {}",
                flag
            ));
    }

    // with subcommands requires dev-deps
    for subcommand in &["test", "bench"] {
        cargo_hack([subcommand, "--remove-dev-deps"])
            .test_dir("tests/fixtures/real")
            .assert_failure()
            .assert_stderr_contains(&format!(
                "--remove-dev-deps may not be used together with {} subcommand",
                subcommand
            ));
    }
}

#[test]
fn ignore_unknown_features() {
    cargo_hack(["check", "--ignore-unknown-features", "--no-default-features", "--features", "f"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "skipped applying unknown `f` feature to member1
             running `cargo check --no-default-features` on member1
             running `cargo check --no-default-features --features f` on member2",
        )
        .assert_stderr_not_contains("skipped applying unknown `f` feature to member2");
}

#[test]
fn ignore_unknown_features_failure() {
    cargo_hack(["check", "--ignore-unknown-features"])
        .test_dir("tests/fixtures/virtual")
        .assert_failure()
        .assert_stderr_contains(
            "
            --ignore-unknown-features can only be used together with --features, --include-features, \
            or --group-features
            ",
        );

    cargo_hack([
        "check",
        "--ignore-unknown-features",
        "--feature-powerset",
        "--include-features",
        "a",
    ])
    .test_dir("tests/fixtures/real")
    .assert_success()
    .assert_stderr_contains(
        "
        --ignore-unknown-features for --include-features is not fully implemented and may not \
        work as intended
        ",
    );

    cargo_hack([
        "check",
        "--ignore-unknown-features",
        "--feature-powerset",
        "--group-features",
        "a,b",
    ])
    .test_dir("tests/fixtures/real")
    .assert_success()
    .assert_stderr_contains(
        "
        --ignore-unknown-features for --group-features is not fully implemented and may not \
        work as intended
        ",
    );
}

#[test]
fn each_feature() {
    cargo_hack(["check", "--each-feature"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/6)
            running `cargo check --no-default-features --features a` on real (2/6)
            running `cargo check --no-default-features --features b` on real (3/6)
            running `cargo check --no-default-features --features c` on real (4/6)
            running `cargo check --no-default-features --features default` on real (5/6)
            running `cargo check --no-default-features --all-features` on real (6/6)
            ",
        );

    // with other feature
    cargo_hack(["check", "--each-feature", "--features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features --features a` on real (1/5)
            running `cargo check --no-default-features --features a,b` on real (2/5)
            running `cargo check --no-default-features --features a,c` on real (3/5)
            running `cargo check --no-default-features --features a,default` on real (4/5)
            running `cargo check --no-default-features --all-features --features a` on real (5/5)
            ",
        )
        .assert_stderr_not_contains("--features a,a");
}

#[test]
fn feature_powerset() {
    cargo_hack(["check", "--feature-powerset"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/17)
            running `cargo check --no-default-features --features a` on real (2/17)
            running `cargo check --no-default-features --features b` on real (3/17)
            running `cargo check --no-default-features --features a,b` on real (4/17)
            running `cargo check --no-default-features --features c` on real (5/17)
            running `cargo check --no-default-features --features a,c` on real (6/17)
            running `cargo check --no-default-features --features b,c` on real (7/17)
            running `cargo check --no-default-features --features a,b,c` on real (8/17)
            running `cargo check --no-default-features --features default` on real (9/17)
            running `cargo check --no-default-features --features a,default` on real (10/17)
            running `cargo check --no-default-features --features b,default` on real (11/17)
            running `cargo check --no-default-features --features a,b,default` on real (12/17)
            running `cargo check --no-default-features --features c,default` on real (13/17)
            running `cargo check --no-default-features --features a,c,default` on real (14/17)
            running `cargo check --no-default-features --features b,c,default` on real (15/17)
            running `cargo check --no-default-features --features a,b,c,default` on real (16/17)
            running `cargo check --no-default-features --all-features` on real (17/17)
            ",
        );

    // with other feature
    cargo_hack(["check", "--feature-powerset", "--features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features --features a` on real (1/9)
            running `cargo check --no-default-features --features a,b` on real (2/9)
            running `cargo check --no-default-features --features a,c` on real (3/9)
            running `cargo check --no-default-features --features a,b,c` on real (4/9)
            running `cargo check --no-default-features --features a,default` on real (5/9)
            running `cargo check --no-default-features --features a,b,default` on real (6/9)
            running `cargo check --no-default-features --features a,c,default` on real (7/9)
            running `cargo check --no-default-features --features a,b,c,default` on real (8/9)
            running `cargo check --no-default-features --all-features --features a` on real (9/9)
            ",
        )
        .assert_stderr_not_contains("--features a,a");
}

#[test]
fn feature_powerset_depth() {
    cargo_hack(["check", "--feature-powerset", "--depth", "2"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/12)
            running `cargo check --no-default-features --features a` on real (2/12)
            running `cargo check --no-default-features --features b` on real (3/12)
            running `cargo check --no-default-features --features a,b` on real (4/12)
            running `cargo check --no-default-features --features c` on real (5/12)
            running `cargo check --no-default-features --features a,c` on real (6/12)
            running `cargo check --no-default-features --features b,c` on real (7/12)
            running `cargo check --no-default-features --features default` on real (8/12)
            running `cargo check --no-default-features --features a,default` on real (9/12)
            running `cargo check --no-default-features --features b,default` on real (10/12)
            running `cargo check --no-default-features --features c,default` on real (11/12)
            running `cargo check --no-default-features --all-features` on real (12/12)
            ",
        )
        .assert_stderr_not_contains("--features a,b,c");
}

#[test]
fn depth_failure() {
    cargo_hack(["check", "--each-feature", "--depth", "2"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains("--depth can only be used together with --feature-powerset");
}

#[test]
fn powerset_group_features() {
    cargo_hack(["check", "--feature-powerset", "--group-features", "a,b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/9)
            running `cargo check --no-default-features --features c` on real (2/9)
            running `cargo check --no-default-features --features default` on real (3/9)
            running `cargo check --no-default-features --features c,default` on real (4/9)
            running `cargo check --no-default-features --features a,b` on real (5/9)
            running `cargo check --no-default-features --features c,a,b` on real (6/9)
            running `cargo check --no-default-features --features default,a,b` on real (7/9)
            running `cargo check --no-default-features --features c,default,a,b` on real (8/9)
            running `cargo check --no-default-features --all-features` on real (9/9)
            ",
        )
        .assert_stderr_not_contains(
            "
            --features a`
            --features b`
            ",
        );

    cargo_hack(["check", "--feature-powerset", "--group-features", "a,b,c"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/5)
            running `cargo check --no-default-features --features default` on real (2/5)
            running `cargo check --no-default-features --features a,b,c` on real (3/5)
            running `cargo check --no-default-features --features default,a,b,c` on real (4/5)
            running `cargo check --no-default-features --all-features` on real (5/5)
            ",
        )
        .assert_stderr_not_contains(
            "
            --features a`
            --features b`
            --features c`
            ",
        );

    // overlapping
    // TODO: Maybe we should warn this, but allow it for now.
    cargo_hack([
        "check",
        "--feature-powerset",
        "--group-features",
        "a,b",
        "--group-features",
        "a,c",
    ])
    .test_dir("tests/fixtures/real")
    .assert_success()
    .assert_stderr_contains(
        "
        running `cargo check --no-default-features` on real (1/9)
        running `cargo check --no-default-features --features default` on real (2/9)
        running `cargo check --no-default-features --features a,b` on real (3/9)
        running `cargo check --no-default-features --features default,a,b` on real (4/9)
        running `cargo check --no-default-features --features a,c` on real (5/9)
        running `cargo check --no-default-features --features default,a,c` on real (6/9)
        running `cargo check --no-default-features --features a,b,a,c` on real (7/9)
        running `cargo check --no-default-features --features default,a,b,a,c` on real (8/9)
        running `cargo check --no-default-features --all-features` on real (9/9)
        ",
    )
    .assert_stderr_not_contains(
        "
        --features a`
        --features b`
        --features c`
        ",
    );
}

#[test]
fn group_features_failure() {
    cargo_hack(["check", "--each-feature", "--group-features", "a,b"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--group-features can only be used together with --feature-powerset",
        );

    cargo_hack(["check", "--feature-powerset", "--group-features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--group-features requires a list of two or more features separated by space or comma",
        );
}

#[test]
fn include_features() {
    cargo_hack(["check", "--each-feature", "--include-features", "a,b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on real (1/2)
             running `cargo check --no-default-features --features b` on real (2/2)",
        )
        .assert_stderr_not_contains("--features c");

    cargo_hack(["check", "--feature-powerset", "--include-features", "a,b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check --no-default-features --features a` on real (1/3)
             running `cargo check --no-default-features --features b` on real (2/3)
             running `cargo check --no-default-features --features a,b` on real (3/3)",
        );
}

#[test]
fn exclude_features_failure() {
    cargo_hack(["check", "--exclude-features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--exclude-features (--skip) can only be used together with either --each-feature or --feature-powerset",
        );

    cargo_hack(["check", "--each-feature", "--exclude-features=a", "--features=a"])
        .test_dir("tests/fixtures/real")
        .assert_success() // warn
        .assert_stderr_contains("feature `a` specified by both --exclude-features and --features");

    cargo_hack([
        "check",
        "--each-feature",
        "--exclude-features=member1",
        "--optional-deps=member1",
    ])
    .test_dir("tests/fixtures/real")
    .assert_success() // warn
    .assert_stderr_contains(
        "feature `member1` specified by both --exclude-features and --optional-deps",
    );

    cargo_hack(["check", "--feature-powerset", "--exclude-features=a", "--group-features=a,b"])
        .test_dir("tests/fixtures/real")
        .assert_success() // warn
        .assert_stderr_contains(
            "feature `a` specified by both --exclude-features and --group-features",
        );

    cargo_hack(["check", "--each-feature", "--exclude-features=a", "--include-features=a,b"])
        .test_dir("tests/fixtures/real")
        .assert_success() // warn
        .assert_stderr_contains(
            "feature `a` specified by both --exclude-features and --include-features",
        );

    cargo_hack(["check", "--each-feature", "--exclude-features=z"])
        .test_dir("tests/fixtures/real")
        .assert_success() // warn
        .assert_stderr_contains("specified feature `z` not found in package `real`");
}

#[test]
fn each_feature_skip_success() {
    cargo_hack(["check", "--each-feature", "--exclude-features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/4)
            running `cargo check --no-default-features --features b` on real (2/4)
            running `cargo check --no-default-features --features c` on real (3/4)
            running `cargo check --no-default-features --features default` on real (4/4)
            ",
        )
        .assert_stderr_not_contains("--features a");

    cargo_hack(["check", "--each-feature", "--exclude-features", "a b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/3)
            running `cargo check --no-default-features --features c` on real (2/3)
            running `cargo check --no-default-features --features default` on real (3/3)
            ",
        )
        .assert_stderr_not_contains(
            "--features a
             --features b",
        );

    cargo_hack(["check", "--each-feature", "--exclude-features", "a", "--exclude-features", "b"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/3)
            running `cargo check --no-default-features --features c` on real (2/3)
            running `cargo check --no-default-features --features default` on real (3/3)
            ",
        )
        .assert_stderr_not_contains(
            "--features a
             --features b",
        );
}

#[test]
fn powerset_skip_success() {
    cargo_hack(["check", "--feature-powerset", "--exclude-features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/8)
            running `cargo check --no-default-features --features b` on real (2/8)
            running `cargo check --no-default-features --features c` on real (3/8)
            running `cargo check --no-default-features --features b,c` on real (4/8)
            running `cargo check --no-default-features --features default` on real (5/8)
            running `cargo check --no-default-features --features b,default` on real (6/8)
            running `cargo check --no-default-features --features c,default` on real (7/8)
            running `cargo check --no-default-features --features b,c,default` on real (8/8)
            ",
        )
        .assert_stderr_not_contains(
            "--features a
             --features a,b
             --features a,c
             --features a,b,c",
        );
}

#[test]
fn exclude_features_default() {
    cargo_hack(["check", "--each-feature", "--exclude-features", "default"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_not_contains("running `cargo check` on real")
        .assert_stderr_contains(
            "running `cargo check --no-default-features` on real (1/4)
             running `cargo check --no-default-features --features a` on real (2/4)
             running `cargo check --no-default-features --features b` on real (3/4)
             running `cargo check --no-default-features --features c` on real (4/4)",
        );
}

#[test]
fn exclude_no_default_features() {
    cargo_hack(["check", "--each-feature", "--exclude-no-default-features"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features --features a` on real (1/5)
            running `cargo check --no-default-features --features b` on real (2/5)
            running `cargo check --no-default-features --features c` on real (3/5)
            running `cargo check --no-default-features --features default` on real (4/5)
            running `cargo check --no-default-features --all-features` on real (5/5)
            ",
        )
        .assert_stderr_not_contains("running `cargo check --no-default-features` on real");

    // --skip-no-default-features is a deprecated alias of --exclude-no-default-features
    cargo_hack(["check", "--each-feature", "--skip-no-default-features"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "--skip-no-default-features is deprecated, use --exclude-no-default-features flag instead",
        );
}

#[test]
fn exclude_no_default_features_failure() {
    cargo_hack(["check", "--exclude-no-default-features"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--exclude-no-default-features can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn exclude_all_features() {
    cargo_hack(["check", "--each-feature", "--exclude-all-features"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/5)
            running `cargo check --no-default-features --features a` on real (2/5)
            running `cargo check --no-default-features --features b` on real (3/5)
            running `cargo check --no-default-features --features c` on real (4/5)
            running `cargo check --no-default-features --features default` on real (5/5)
            ",
        )
        .assert_stderr_not_contains(
            "running `cargo check --no-default-features --all-features` on real",
        );
}

#[test]
fn exclude_all_features_failure() {
    cargo_hack(["check", "--exclude-all-features"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--exclude-all-features can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn each_feature_all() {
    cargo_hack(["check", "--each-feature", "--workspace"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on member1 (1/24)
            running `cargo check --no-default-features --features a` on member1 (2/24)
            running `cargo check --no-default-features --features b` on member1 (3/24)
            running `cargo check --no-default-features --features c` on member1 (4/24)
            running `cargo check --no-default-features --features default` on member1 (5/24)
            running `cargo check --no-default-features --all-features` on member1 (6/24)
            running `cargo check --no-default-features` on member2 (7/24)
            running `cargo check --no-default-features --features a` on member2 (8/24)
            running `cargo check --no-default-features --features b` on member2 (9/24)
            running `cargo check --no-default-features --features c` on member2 (10/24)
            running `cargo check --no-default-features --features default` on member2 (11/24)
            running `cargo check --no-default-features --all-features` on member2 (12/24)
            running `cargo check --no-default-features` on member3 (13/24)
            running `cargo check --no-default-features --features a` on member3 (14/24)
            running `cargo check --no-default-features --features b` on member3 (15/24)
            running `cargo check --no-default-features --features c` on member3 (16/24)
            running `cargo check --no-default-features --features default` on member3 (17/24)
            running `cargo check --no-default-features --all-features` on member3 (18/24)
            running `cargo check --no-default-features` on real (19/24)
            running `cargo check --no-default-features --features a` on real (20/24)
            running `cargo check --no-default-features --features b` on real (21/24)
            running `cargo check --no-default-features --features c` on real (22/24)
            running `cargo check --no-default-features --features default` on real (23/24)
            running `cargo check --no-default-features --all-features` on real (24/24)
            ",
        );
}

#[test]
fn include_deps_features() {
    cargo_hack(["check", "--each-feature", "--include-deps-features"])
        .test_dir("tests")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on cargo-hack (1/20)
            running `cargo check --no-default-features --features anyhow/default` on cargo-hack (2/20)
            running `cargo check --no-default-features --features anyhow/std` on cargo-hack (3/20)
            running `cargo check --no-default-features --features ctrlc/termination` on cargo-hack (4/20)
            running `cargo check --no-default-features --features serde_json/alloc` on cargo-hack (5/20)
            running `cargo check --no-default-features --features serde_json/arbitrary_precision` on cargo-hack (6/20)
            running `cargo check --no-default-features --features serde_json/default` on cargo-hack (7/20)
            running `cargo check --no-default-features --features serde_json/float_roundtrip` on cargo-hack (8/20)
            running `cargo check --no-default-features --features serde_json/preserve_order` on cargo-hack (9/20)
            running `cargo check --no-default-features --features serde_json/raw_value` on cargo-hack (10/20)
            running `cargo check --no-default-features --features serde_json/std` on cargo-hack (11/20)
            running `cargo check --no-default-features --features serde_json/unbounded_depth` on cargo-hack (12/20)
            running `cargo check --no-default-features --features term_size/debug` on cargo-hack (13/20)
            running `cargo check --no-default-features --features term_size/default` on cargo-hack (14/20)
            running `cargo check --no-default-features --features term_size/nightly` on cargo-hack (15/20)
            running `cargo check --no-default-features --features term_size/travis` on cargo-hack (16/20)
            running `cargo check --no-default-features --features term_size/unstable` on cargo-hack (17/20)
            running `cargo check --no-default-features --features toml/default` on cargo-hack (18/20)
            running `cargo check --no-default-features --features toml/preserve_order` on cargo-hack (19/20)
            running `cargo check --no-default-features --all-features` on cargo-hack (20/20)
            ",
        );
}

#[rustversion::attr(not(before(1.41)), ignore)]
#[test]
fn include_deps_features_version_failure() {
    cargo_hack(["check", "--each-feature", "--include-deps-features", "--strict-metadata-version"])
        .test_dir("tests")
        .assert_failure()
        .assert_stderr_contains("--include-deps-features requires Cargo 1.41 or leter");
}

#[test]
fn trailing_args() {
    cargo_hack(["test", "--", "--ignored"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("running `cargo test -- --ignored` on real")
        .assert_stdout_contains(
            "running 1 test
             test tests::test_ignored",
        );
}

#[test]
fn package_collision() {
    cargo_hack(["check"])
        .test_dir("tests/fixtures/package_collision")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2",
        );
}

#[test]
fn not_find_manifest() {
    cargo_hack(["check"])
        .test_dir("tests/fixtures/virtual/dir/not_find_manifest")
        .assert_success()
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             running `cargo check` on member2",
        )
        .assert_stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack(["check", "--all"])
        .test_dir("tests/fixtures/virtual/dir/not_find_manifest")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2
             running `cargo check` on not_find_manifest",
        );

    cargo_hack(["check", "--manifest-path", "dir/not_find_manifest/Cargo.toml"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_not_contains(
            "running `cargo check` on member1
             running `cargo check` on member2",
        )
        .assert_stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack(["check", "--all", "--manifest-path", "dir/not_find_manifest/Cargo.toml"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(
            "running `cargo check` on member1
             running `cargo check` on member2
             running `cargo check` on not_find_manifest",
        );
}

#[test]
fn optional_deps() {
    cargo_hack(["run", "--features=real,member2,renemed", "--ignore-unknown-features"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "skipped applying unknown `member2` feature to optional_deps
             running `cargo run --features real,renemed` on optional_deps",
        )
        .assert_stdout_contains(
            "renemed
             real",
        )
        .assert_stdout_not_contains(
            "member3
             member2",
        );

    cargo_hack(["check", "--each-feature"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on optional_deps (1/1)")
        .assert_stderr_not_contains(
            "--no-default-features
             --features real
             --features renemed",
        );

    cargo_hack(["check", "--each-feature", "--optional-deps"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/4)
            running `cargo check --no-default-features --features real` on optional_deps (2/4)
            running `cargo check --no-default-features --features renemed` on optional_deps (3/4)
            running `cargo check --no-default-features --all-features` on optional_deps (4/4)
            ",
        );

    cargo_hack(["check", "--each-feature", "--optional-deps", "real"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/3)
            running `cargo check --no-default-features --features real` on optional_deps (2/3)
            running `cargo check --no-default-features --all-features` on optional_deps (3/3)
            ",
        )
        .assert_stderr_not_contains("--features renemed");

    cargo_hack(["check", "--each-feature", "--optional-deps=renemed"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/3)
            running `cargo check --no-default-features --features renemed` on optional_deps (2/3)
            running `cargo check --no-default-features --all-features` on optional_deps (3/3)
            ",
        )
        .assert_stderr_not_contains("--features real");

    cargo_hack(["check", "--each-feature", "--optional-deps="])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo check` on optional_deps (1/1)");
}

#[test]
fn optional_deps_failure() {
    cargo_hack(["check", "--optional-deps"])
        .test_dir("tests/fixtures/real")
        .assert_failure()
        .assert_stderr_contains(
            "--optional-deps can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn skip_optional_deps() {
    cargo_hack(["check", "--each-feature", "--optional-deps", "--exclude-features", "real"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/2)
            running `cargo check --no-default-features --features renemed` on optional_deps (2/2)
            ",
        )
        .assert_stderr_not_contains("--features real");
}

#[test]
fn list_separator() {
    cargo_hack(["run", "--features='real,renemed'"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features=\"real,renemed\""])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features=real,renemed"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features", "real,renemed"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features='real renemed'"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features=\"real renemed\""])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features", "real renemed"])
        .test_dir("tests/fixtures/optional_deps")
        .assert_success()
        .assert_stderr_contains("running `cargo run --features real,renemed` on optional_deps");
}

#[test]
fn verbose() {
    cargo_hack(["check", "--verbose"])
        .test_dir("tests/fixtures/virtual")
        .assert_success()
        .assert_stderr_contains(&format!(
            "running `cargo check --manifest-path member1{0}Cargo.toml`
             running `cargo check --manifest-path member2{0}Cargo.toml`
             running `cargo check --manifest-path dir{0}not_find_manifest{0}Cargo.toml`",
            SEPARATOR
        ));
}

#[test]
fn propagate() {
    // --features
    cargo_hack(["check", "--features", "a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("--features a");
    cargo_hack(["check", "--features=a"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("--features a");

    // --no-default-features
    cargo_hack(["check", "--no-default-features"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("--no-default-features");

    // --all-features
    cargo_hack(["check", "--all-features"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("--all-features");

    // --color
    cargo_hack(["check", "--color", "auto"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("`cargo check --color auto`");
    cargo_hack(["check", "--color=auto"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_contains("`cargo check --color=auto`");

    // --verbose does not be propagated
    cargo_hack(["check", "--verbose"])
        .test_dir("tests/fixtures/real")
        .assert_success()
        .assert_stderr_not_contains("--verbose");
}

#[test]
fn default_feature_behavior() {
    cargo_hack(["run"])
        .test_dir("tests/fixtures/default_feature_behavior/has_default")
        .assert_success()
        .assert_stdout_contains("has default feature!")
        .assert_stdout_not_contains("no default feature!");

    cargo_hack(["run", "--no-default-features"])
        .test_dir("tests/fixtures/default_feature_behavior/has_default")
        .assert_success()
        .assert_stdout_contains("no default feature!")
        .assert_stdout_not_contains("has default feature!");

    cargo_hack(["run"])
        .test_dir("tests/fixtures/default_feature_behavior/no_default")
        .assert_success()
        .assert_stdout_contains("no default feature!")
        .assert_stdout_not_contains("has default feature!");

    cargo_hack(["run", "--no-default-features"])
        .test_dir("tests/fixtures/default_feature_behavior/no_default")
        .assert_success()
        .assert_stdout_contains("no default feature!")
        .assert_stdout_not_contains("has default feature!");
}
