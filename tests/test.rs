#![warn(rust_2018_idioms, single_use_lifetimes)]

mod auxiliary;

use std::{
    env::{self, consts::EXE_SUFFIX},
    path::MAIN_SEPARATOR,
};

use auxiliary::{cargo_bin_exe, cargo_hack, has_stable_toolchain, CommandExt, TARGET};

#[test]
fn failures() {
    cargo_bin_exe().assert_failure("real");

    cargo_bin_exe()
        .arg("--all")
        .assert_failure("real")
        .stderr_contains("expected subcommand 'hack', found argument '--all'");

    cargo_hack([] as [&str; 0])
        .assert_failure("real")
        .stderr_contains("no subcommand or valid flag specified");

    cargo_hack(["--all"])
        .assert_failure("real")
        .stderr_contains("no subcommand or valid flag specified");

    cargo_hack(["install"])
        .assert_failure("real")
        .stderr_contains("cargo-hack may not be used together with install subcommand");
}

#[test]
fn multi_arg() {
    for flag in &[
        "--workspace",
        "--all",
        "--each-feature",
        "--feature-powerset",
        "--no-dev-deps",
        "--remove-dev-deps",
        "--ignore-private",
        "--ignore-unknown-features",
        "--optional-deps",
        "--manifest-path=foo",
        "--color=auto",
    ] {
        cargo_hack(["check", flag, flag]).assert_failure("real").stderr_contains(format!(
            "The argument '{}' was provided more than once, but cannot be used multiple times",
            flag.split('=').next().unwrap()
        ));
    }
}

#[test]
fn removed_flags() {
    for (flag, alt) in &[
        ("--ignore-non-exist-features", "--ignore-unknown-features"),
        ("--skip-no-default-features", "--exclude-no-default-features"),
    ] {
        cargo_hack(["check", flag])
            .assert_failure("real")
            .stderr_contains(format!("{flag} was removed, use {alt} instead"));
    }
}

#[test]
#[ignore = "the order of the operation remains different each time"]
fn real_manifest() {
    cargo_hack(["check"])
        .assert_success("real")
        .stderr_not_contains(
            "
            running `cargo check` on member1
            running `cargo check` on member2
            running `cargo check` on member3
            ",
        )
        .stderr_contains("running `cargo check` on real");

    cargo_hack(["check", "--workspace"]).assert_success("real").stderr_contains(
        "
        running `cargo check` on member1 (1/4)
        running `cargo check` on member2 (2/4)
        running `cargo check` on member3 (3/4)
        running `cargo check` on real (4/4)
        ",
    );
}

#[test]
fn virtual_manifest() {
    cargo_hack(["check"]).assert_success("virtual").stderr_contains(
        "
        running `cargo check` on member1 (1/3)
        running `cargo check` on member2 (2/3)
        ",
    );

    cargo_hack(["check", "--all"]).assert_success("virtual").stderr_contains(
        "
        running `cargo check` on member1 (1/3)
        running `cargo check` on member2 (2/3)
        ",
    );
}

#[test]
fn real_all_in_subcrate() {
    cargo_hack(["check"])
        .assert_success("real/member2")
        .stderr_contains("running `cargo check` on member2")
        .stderr_not_contains(
            "
            running `cargo check` on member1
            running `cargo check` on member3
            running `cargo check` on real
            ",
        );

    cargo_hack(["check", "--all"]).assert_success("real/member2").stderr_contains(
        "
        running `cargo check` on member1
        running `cargo check` on member2
        running `cargo check` on member3
        running `cargo check` on real
        ",
    );
}

#[test]
fn virtual_all_in_subcrate() {
    cargo_hack(["check"])
        .assert_success("virtual/member1")
        .stderr_contains("running `cargo check` on member1")
        .stderr_not_contains("running `cargo check` on member2");

    cargo_hack(["check", "--all"]).assert_success("virtual/member1").stderr_contains(
        "
        running `cargo check` on member1
        running `cargo check` on member2
        ",
    );
}

#[test]
fn real_ignore_private() {
    cargo_hack(["check", "--ignore-private"])
        .assert_success("real")
        .stderr_not_contains(
            "
            running `cargo check` on member1
            skipped running on private package `member1`
            running `cargo check` on member2
            skipped running on private package `member2`
            running `cargo check` on real
            ",
        )
        .stderr_contains("skipped running on private package `real`");

    cargo_hack(["check", "--all", "--ignore-private"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check` on member1
            skipped running on private package `member2`
            running `cargo check` on member3
            skipped running on private package `real`
            ",
        )
        .stderr_not_contains(
            "
            skipped running on private package `member1`
            running `cargo check` on member2
            skipped running on private package `member3`
            running `cargo check` on real
            ",
        );
}

#[test]
fn virtual_ignore_private() {
    cargo_hack(["check", "--ignore-private"])
        .assert_success("virtual")
        .stderr_contains(
            "
            running `cargo check` on member1
            skipped running on private package `member2`
            ",
        )
        .stderr_not_contains(
            "
            skipped running on private package `member1`
            running `cargo check` on member2
            ",
        );

    cargo_hack(["check", "--all", "--ignore-private"])
        .assert_success("virtual")
        .stderr_contains(
            "
            running `cargo check` on member1
            skipped running on private package `member2`
            ",
        )
        .stderr_not_contains(
            "
            running `cargo check` on member2
            skipped running on private package `member1`
            ",
        );
}

#[test]
fn package() {
    cargo_hack(["check", "--package", "member1"])
        .assert_success("virtual")
        .stderr_contains("running `cargo check` on member1")
        .stderr_not_contains("running `cargo check` on member2");
}

#[test]
fn package_no_packages() {
    cargo_hack(["check", "--package", "foo"])
        .assert_failure("virtual")
        .stderr_contains("package ID specification `foo` matched no packages");
}

#[test]
fn exclude() {
    cargo_hack(["check", "--all", "--exclude", "member1"])
        .assert_success("virtual")
        .stderr_not_contains("running `cargo check` on member1")
        .stderr_contains("running `cargo check` on member2");

    // not_found is warning
    cargo_hack(["check", "--all", "--exclude", "foo"]).assert_failure("virtual").stderr_contains(
        "
        excluded package(s) `foo` not found in workspace
        running `cargo check` on member1
        running `cargo check` on member2
        ",
    );
}

#[test]
fn exclude_failure() {
    // not with --workspace
    cargo_hack(["check", "--exclude", "member1"])
        .assert_failure("virtual")
        .stderr_contains("--exclude can only be used together with --workspace");
}

#[test]
fn no_dev_deps() {
    cargo_hack(["check", "--no-dev-deps"]).assert_success("real").stderr_contains(
        "
        running `cargo check` on real
        --no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is \
        running and restores it when finished
        ",
    );

    // with --all
    cargo_hack(["check", "--no-dev-deps", "--all"]).assert_success("real").stderr_contains(
        "
        --no-dev-deps removes dev-dependencies from real `Cargo.toml` while cargo-hack is \
        running and restores it when finished
        ",
    );
}

#[test]
fn no_dev_deps_failure() {
    // with --remove-dev-deps
    cargo_hack(["check", "--no-dev-deps", "--remove-dev-deps"])
        .assert_failure("real")
        .stderr_contains("--no-dev-deps may not be used together with --remove-dev-deps");

    // with options requires dev-deps
    for flag in
        &["--example", "--examples", "--test", "--tests", "--bench", "--benches", "--all-targets"]
    {
        cargo_hack(["check", "--no-dev-deps", flag])
            .assert_failure("real")
            .stderr_contains(format!("--no-dev-deps may not be used together with {flag}"));
    }

    // with subcommands requires dev-deps
    for subcommand in &["test", "bench"] {
        cargo_hack([subcommand, "--no-dev-deps"]).assert_failure("real").stderr_contains(format!(
            "--no-dev-deps may not be used together with {subcommand} subcommand"
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
            .assert_failure("real")
            .stderr_contains(format!("--remove-dev-deps may not be used together with {flag}"));
    }

    // with subcommands requires dev-deps
    for subcommand in &["test", "bench"] {
        cargo_hack([subcommand, "--remove-dev-deps"]).assert_failure("real").stderr_contains(
            format!("--remove-dev-deps may not be used together with {subcommand} subcommand"),
        );
    }
}

#[test]
fn ignore_unknown_features() {
    cargo_hack(["check", "--ignore-unknown-features", "--no-default-features", "--features", "f"])
        .assert_success("virtual")
        .stderr_contains(
            "
            skipped applying unknown `f` feature to member1
            running `cargo check --no-default-features` on member1
            running `cargo check --no-default-features --features f` on member2
            ",
        )
        .stderr_not_contains("skipped applying unknown `f` feature to member2");
}

#[test]
fn ignore_unknown_features_failure() {
    cargo_hack(["check", "--ignore-unknown-features"])
        .assert_failure("virtual")
        .stderr_contains(
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
    .assert_success("real")
    .stderr_contains(
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
    .assert_success("real")
    .stderr_contains(
        "
        --ignore-unknown-features for --group-features is not fully implemented and may not \
        work as intended
        ",
    );
}

#[test]
fn each_feature() {
    cargo_hack(["check", "--each-feature"]).assert_success("real").stderr_contains(
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
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features --features a` on real (1/5)
            running `cargo check --no-default-features --features a,b` on real (2/5)
            running `cargo check --no-default-features --features a,c` on real (3/5)
            running `cargo check --no-default-features --features a,default` on real (4/5)
            running `cargo check --no-default-features --all-features --features a` on real (5/5)
            ",
        )
        .stderr_not_contains("--features a,a");
}

#[test]
fn each_feature_failure() {
    cargo_hack(["check", "--each-feature", "--feature-powerset"])
        .assert_failure("real")
        .stderr_contains("--each-feature may not be used together with --feature-powerset");

    cargo_hack(["check", "--each-feature", "--all-features"])
        .assert_failure("real")
        .stderr_contains("--all-features may not be used together with --each-feature");

    cargo_hack(["check", "--each-feature", "--no-default-features"])
        .assert_failure("real")
        .stderr_contains("--no-default-features may not be used together with --each-feature");
}

#[test]
fn feature_powerset() {
    cargo_hack(["check", "--feature-powerset"]).assert_success("real").stderr_contains(
        "
        running `cargo check --no-default-features` on real (1/16)
        running `cargo check --no-default-features --features a` on real (2/16)
        running `cargo check --no-default-features --features b` on real (3/16)
        running `cargo check --no-default-features --features a,b` on real (4/16)
        running `cargo check --no-default-features --features c` on real (5/16)
        running `cargo check --no-default-features --features a,c` on real (6/16)
        running `cargo check --no-default-features --features b,c` on real (7/16)
        running `cargo check --no-default-features --features a,b,c` on real (8/16)
        running `cargo check --no-default-features --features default` on real (9/16)
        running `cargo check --no-default-features --features a,default` on real (10/16)
        running `cargo check --no-default-features --features b,default` on real (11/16)
        running `cargo check --no-default-features --features a,b,default` on real (12/16)
        running `cargo check --no-default-features --features c,default` on real (13/16)
        running `cargo check --no-default-features --features a,c,default` on real (14/16)
        running `cargo check --no-default-features --features b,c,default` on real (15/16)
        running `cargo check --no-default-features --features a,b,c,default` on real (16/16)
        ",
    );

    // with other feature
    cargo_hack(["check", "--feature-powerset", "--features", "a"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features --features a` on real (1/8)
            running `cargo check --no-default-features --features a,b` on real (2/8)
            running `cargo check --no-default-features --features a,c` on real (3/8)
            running `cargo check --no-default-features --features a,b,c` on real (4/8)
            running `cargo check --no-default-features --features a,default` on real (5/8)
            running `cargo check --no-default-features --features a,b,default` on real (6/8)
            running `cargo check --no-default-features --features a,c,default` on real (7/8)
            running `cargo check --no-default-features --features a,b,c,default` on real (8/8)
            ",
        )
        .stderr_not_contains("--features a,a");
}

#[test]
fn feature_powerset_failure() {
    cargo_hack(["check", "--each-feature", "--feature-powerset"])
        .assert_failure("real")
        .stderr_contains("--each-feature may not be used together with --feature-powerset");

    cargo_hack(["check", "--feature-powerset", "--all-features"])
        .assert_failure("real")
        .stderr_contains("--all-features may not be used together with --feature-powerset");

    cargo_hack(["check", "--feature-powerset", "--no-default-features"])
        .assert_failure("real")
        .stderr_contains("--no-default-features may not be used together with --feature-powerset");
}

#[test]
fn powerset_deduplication() {
    // basic
    // require Rust 1.34 due to easytime requires it.
    cargo_hack(["check", "--feature-powerset"])
        .assert_success2("powerset_deduplication", Some(34))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on deduplication (1/10)
            running `cargo check --no-default-features --features a` on deduplication (2/10)
            running `cargo check --no-default-features --features b` on deduplication (3/10)
            running `cargo check --no-default-features --features c` on deduplication (4/10)
            running `cargo check --no-default-features --features d` on deduplication (5/10)
            running `cargo check --no-default-features --features a,d` on deduplication (6/10)
            running `cargo check --no-default-features --features b,d` on deduplication (7/10)
            running `cargo check --no-default-features --features c,d` on deduplication (8/10)
            running `cargo check --no-default-features --features e` on deduplication (9/10)
            running `cargo check --no-default-features --features c,e` on deduplication (10/10)
            ",
        )
        .stderr_not_contains(
            "
            a,b
            b,c
            a,c
            a,e
            b,e
            d,e
            ",
        );

    // with --optional-deps
    cargo_hack(["check", "--feature-powerset", "--optional-deps"])
        .assert_success2("powerset_deduplication", Some(34))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on deduplication (1/14)
            running `cargo check --no-default-features --features a` on deduplication (2/14)
            running `cargo check --no-default-features --features b` on deduplication (3/14)
            running `cargo check --no-default-features --features c` on deduplication (4/14)
            running `cargo check --no-default-features --features d` on deduplication (5/14)
            running `cargo check --no-default-features --features a,d` on deduplication (6/14)
            running `cargo check --no-default-features --features b,d` on deduplication (7/14)
            running `cargo check --no-default-features --features c,d` on deduplication (8/14)
            running `cargo check --no-default-features --features e` on deduplication (9/14)
            running `cargo check --no-default-features --features c,e` on deduplication (10/14)
            running `cargo check --no-default-features --features member1` on deduplication (11/14)
            running `cargo check --no-default-features --features a,member1` on deduplication (12/14)
            running `cargo check --no-default-features --features b,member1` on deduplication (13/14)
            running `cargo check --no-default-features --features c,member1` on deduplication (14/14)
            ",
        )
        .stderr_not_contains(
            "
            a,b
            b,c
            a,c
            a,e
            b,e
            d,e
            ",
        );

    // with --group-features
    cargo_hack(["check", "--feature-powerset", "--group-features", "b,d"])
        .assert_success2("powerset_deduplication", Some(34))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on deduplication (1/7)
            running `cargo check --no-default-features --features a` on deduplication (2/7)
            running `cargo check --no-default-features --features c` on deduplication (3/7)
            running `cargo check --no-default-features --features e` on deduplication (4/7)
            running `cargo check --no-default-features --features c,e` on deduplication (5/7)
            running `cargo check --no-default-features --features b,d` on deduplication (6/7)
            running `cargo check --no-default-features --features c,b,d` on deduplication (7/7)
            ",
        )
        .stderr_not_contains(
            "
            a,b,d
            e,b,d
            ",
        );

    // with --group-features + --optional-deps
    cargo_hack(["check", "--feature-powerset", "--group-features", "b,d", "--optional-deps"])
        .assert_success2("powerset_deduplication", Some(34))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on deduplication (1/10)
            running `cargo check --no-default-features --features a` on deduplication (2/10)
            running `cargo check --no-default-features --features c` on deduplication (3/10)
            running `cargo check --no-default-features --features e` on deduplication (4/10)
            running `cargo check --no-default-features --features c,e` on deduplication (5/10)
            running `cargo check --no-default-features --features member1` on deduplication (6/10)
            running `cargo check --no-default-features --features a,member1` on deduplication (7/10)
            running `cargo check --no-default-features --features c,member1` on deduplication (8/10)
            running `cargo check --no-default-features --features b,d` on deduplication (9/10)
            running `cargo check --no-default-features --features c,b,d` on deduplication (10/10)
            ",
        )
        .stderr_not_contains(
            "
            a,b,d
            b,d,a
            e,b,d
            b,d,e
            member1,b,d
            b,d,member1
            ",
        );
}

#[test]
fn powerset_deduplication_include_deps_features() {
    // TODO: Since easytime/default depends on easytime/std, their combination should be excluded,
    // but it's not working yet because include-deps-features itself isn't fully implemented.
    cargo_hack(["check", "--feature-powerset", "--include-deps-features"])
        .assert_success2("powerset_deduplication",  Some(if has_stable_toolchain() { 34 } else { 41 }))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on deduplication (1/40)
            running `cargo check --no-default-features --features a` on deduplication (2/40)
            running `cargo check --no-default-features --features b` on deduplication (3/40)
            running `cargo check --no-default-features --features c` on deduplication (4/40)
            running `cargo check --no-default-features --features d` on deduplication (5/40)
            running `cargo check --no-default-features --features a,d` on deduplication (6/40)
            running `cargo check --no-default-features --features b,d` on deduplication (7/40)
            running `cargo check --no-default-features --features c,d` on deduplication (8/40)
            running `cargo check --no-default-features --features e` on deduplication (9/40)
            running `cargo check --no-default-features --features c,e` on deduplication (10/40)
            running `cargo check --no-default-features --features easytime/default` on deduplication (11/40)
            running `cargo check --no-default-features --features a,easytime/default` on deduplication (12/40)
            running `cargo check --no-default-features --features b,easytime/default` on deduplication (13/40)
            running `cargo check --no-default-features --features c,easytime/default` on deduplication (14/40)
            running `cargo check --no-default-features --features d,easytime/default` on deduplication (15/40)
            running `cargo check --no-default-features --features a,d,easytime/default` on deduplication (16/40)
            running `cargo check --no-default-features --features b,d,easytime/default` on deduplication (17/40)
            running `cargo check --no-default-features --features c,d,easytime/default` on deduplication (18/40)
            running `cargo check --no-default-features --features e,easytime/default` on deduplication (19/40)
            running `cargo check --no-default-features --features c,e,easytime/default` on deduplication (20/40)
            running `cargo check --no-default-features --features easytime/std` on deduplication (21/40)
            running `cargo check --no-default-features --features a,easytime/std` on deduplication (22/40)
            running `cargo check --no-default-features --features b,easytime/std` on deduplication (23/40)
            running `cargo check --no-default-features --features c,easytime/std` on deduplication (24/40)
            running `cargo check --no-default-features --features d,easytime/std` on deduplication (25/40)
            running `cargo check --no-default-features --features a,d,easytime/std` on deduplication (26/40)
            running `cargo check --no-default-features --features b,d,easytime/std` on deduplication (27/40)
            running `cargo check --no-default-features --features c,d,easytime/std` on deduplication (28/40)
            running `cargo check --no-default-features --features e,easytime/std` on deduplication (29/40)
            running `cargo check --no-default-features --features c,e,easytime/std` on deduplication (30/40)
            running `cargo check --no-default-features --features easytime/default,easytime/std` on deduplication (31/40)
            running `cargo check --no-default-features --features a,easytime/default,easytime/std` on deduplication (32/40)
            running `cargo check --no-default-features --features b,easytime/default,easytime/std` on deduplication (33/40)
            running `cargo check --no-default-features --features c,easytime/default,easytime/std` on deduplication (34/40)
            running `cargo check --no-default-features --features d,easytime/default,easytime/std` on deduplication (35/40)
            running `cargo check --no-default-features --features a,d,easytime/default,easytime/std` on deduplication (36/40)
            running `cargo check --no-default-features --features b,d,easytime/default,easytime/std` on deduplication (37/40)
            running `cargo check --no-default-features --features c,d,easytime/default,easytime/std` on deduplication (38/40)
            running `cargo check --no-default-features --features e,easytime/default,easytime/std` on deduplication (39/40)
            running `cargo check --no-default-features --features c,e,easytime/default,easytime/std` on deduplication (40/40)
            ",
        );
}

#[test]
fn feature_powerset_depth() {
    cargo_hack(["check", "--feature-powerset", "--depth", "2"])
        .assert_success("real")
        .stderr_contains(
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
        .stderr_not_contains("--features a,b,c");
}

#[test]
fn depth_failure() {
    cargo_hack(["check", "--each-feature", "--depth", "2"])
        .assert_failure("real")
        .stderr_contains("--depth can only be used together with --feature-powerset");
}

#[test]
fn powerset_group_features() {
    cargo_hack(["check", "--feature-powerset", "--group-features", "a,b"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/8)
            running `cargo check --no-default-features --features c` on real (2/8)
            running `cargo check --no-default-features --features default` on real (3/8)
            running `cargo check --no-default-features --features c,default` on real (4/8)
            running `cargo check --no-default-features --features a,b` on real (5/8)
            running `cargo check --no-default-features --features c,a,b` on real (6/8)
            running `cargo check --no-default-features --features default,a,b` on real (7/8)
            running `cargo check --no-default-features --features c,default,a,b` on real (8/8)
            ",
        )
        .stderr_not_contains(
            "
            --features a`
            --features b`
            ",
        );

    cargo_hack(["check", "--feature-powerset", "--group-features", "a,b,c"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/4)
            running `cargo check --no-default-features --features default` on real (2/4)
            running `cargo check --no-default-features --features a,b,c` on real (3/4)
            running `cargo check --no-default-features --features default,a,b,c` on real (4/4)
            ",
        )
        .stderr_not_contains(
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
    .assert_success("real")
    .stderr_contains(
        "
        running `cargo check --no-default-features` on real (1/8)
        running `cargo check --no-default-features --features default` on real (2/8)
        running `cargo check --no-default-features --features a,b` on real (3/8)
        running `cargo check --no-default-features --features default,a,b` on real (4/8)
        running `cargo check --no-default-features --features a,c` on real (5/8)
        running `cargo check --no-default-features --features default,a,c` on real (6/8)
        running `cargo check --no-default-features --features a,b,a,c` on real (7/8)
        running `cargo check --no-default-features --features default,a,b,a,c` on real (8/8)
        ",
    )
    .stderr_not_contains(
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
        .assert_failure("real")
        .stderr_contains("--group-features can only be used together with --feature-powerset");

    cargo_hack(["check", "--feature-powerset", "--group-features", "a"])
        .assert_failure("real")
        .stderr_contains(
            "--group-features requires a list of two or more features separated by space or comma",
        );
}

#[test]
fn include_features() {
    cargo_hack(["check", "--each-feature", "--include-features", "a,b"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features --features a` on real (1/2)
            running `cargo check --no-default-features --features b` on real (2/2)
            ",
        )
        .stderr_not_contains("--features c");

    cargo_hack(["check", "--feature-powerset", "--include-features", "a,b"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features --features a` on real (1/3)
            running `cargo check --no-default-features --features b` on real (2/3)
            running `cargo check --no-default-features --features a,b` on real (3/3)
            ",
        );
}

#[test]
fn exclude_features() {
    cargo_hack(["check", "--each-feature", "--exclude-features", "f"])
        .assert_success("virtual")
        .stderr_not_contains("specified feature `f` not found");
}

#[test]
fn exclude_features_failure() {
    cargo_hack(["check", "--exclude-features", "a"])
        .assert_failure("real")
        .stderr_contains(
            "--exclude-features (--skip) can only be used together with either --each-feature or --feature-powerset",
        );

    cargo_hack(["check", "--each-feature", "--exclude-features=a", "--features=a"])
        .assert_failure("real")
        .stderr_contains("feature `a` specified by both --exclude-features and --features");

    cargo_hack([
        "check",
        "--each-feature",
        "--exclude-features=member1",
        "--optional-deps=member1",
    ])
    .assert_failure("real")
    .stderr_contains("feature `member1` specified by both --exclude-features and --optional-deps");

    cargo_hack(["check", "--feature-powerset", "--exclude-features=a", "--group-features=a,b"])
        .assert_failure("real")
        .stderr_contains("feature `a` specified by both --exclude-features and --group-features");

    cargo_hack(["check", "--each-feature", "--exclude-features=a", "--include-features=a,b"])
        .assert_failure("real")
        .stderr_contains("feature `a` specified by both --exclude-features and --include-features");

    cargo_hack(["check", "--each-feature", "--exclude-features=z"])
        .assert_failure("real") // warn
        .stderr_contains("specified feature `z` not found in package `real`");
}

#[test]
fn each_feature_skip_success() {
    cargo_hack(["check", "--each-feature", "--exclude-features", "a"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/4)
            running `cargo check --no-default-features --features b` on real (2/4)
            running `cargo check --no-default-features --features c` on real (3/4)
            running `cargo check --no-default-features --features default` on real (4/4)
            ",
        )
        .stderr_not_contains("--features a");

    cargo_hack(["check", "--each-feature", "--exclude-features", "a b"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/3)
            running `cargo check --no-default-features --features c` on real (2/3)
            running `cargo check --no-default-features --features default` on real (3/3)
            ",
        )
        .stderr_not_contains(
            "--features a
             --features b",
        );

    cargo_hack(["check", "--each-feature", "--exclude-features", "a", "--exclude-features", "b"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/3)
            running `cargo check --no-default-features --features c` on real (2/3)
            running `cargo check --no-default-features --features default` on real (3/3)
            ",
        )
        .stderr_not_contains(
            "
            --features a
            --features b
            ",
        );
}

#[test]
fn powerset_skip_success() {
    cargo_hack(["check", "--feature-powerset", "--exclude-features", "a"])
        .assert_success("real")
        .stderr_contains(
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
        .stderr_not_contains(
            "
            --features a
            --features a,b
            --features a,c
            --features a,b,c
            ",
        );
}

#[test]
fn exclude_features_default() {
    cargo_hack(["check", "--each-feature", "--exclude-features", "default"])
        .assert_success("real")
        .stderr_not_contains("running `cargo check` on real")
        .stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/4)
            running `cargo check --no-default-features --features a` on real (2/4)
            running `cargo check --no-default-features --features b` on real (3/4)
            running `cargo check --no-default-features --features c` on real (4/4)
            ",
        );
}

#[test]
fn exclude_no_default_features() {
    cargo_hack(["check", "--each-feature", "--exclude-no-default-features"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features --features a` on real (1/5)
            running `cargo check --no-default-features --features b` on real (2/5)
            running `cargo check --no-default-features --features c` on real (3/5)
            running `cargo check --no-default-features --features default` on real (4/5)
            running `cargo check --no-default-features --all-features` on real (5/5)
            ",
        )
        .stderr_not_contains("running `cargo check --no-default-features` on real");
}

#[test]
fn exclude_no_default_features_failure() {
    cargo_hack(["check", "--exclude-no-default-features"])
        .assert_failure("real")
        .stderr_contains(
            "--exclude-no-default-features can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn exclude_all_features() {
    cargo_hack(["check", "--each-feature", "--exclude-all-features"])
        .assert_success("real")
        .stderr_contains(
            "
            running `cargo check --no-default-features` on real (1/5)
            running `cargo check --no-default-features --features a` on real (2/5)
            running `cargo check --no-default-features --features b` on real (3/5)
            running `cargo check --no-default-features --features c` on real (4/5)
            running `cargo check --no-default-features --features default` on real (5/5)
            ",
        )
        .stderr_not_contains("running `cargo check --no-default-features --all-features` on real");
}

#[test]
fn exclude_all_features_failure() {
    cargo_hack(["check", "--exclude-all-features"])
        .assert_failure("real")
        .stderr_contains(
            "--exclude-all-features can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
#[ignore = "The commands run in different order each time"]
fn each_feature_all() {
    cargo_hack(["check", "--each-feature", "--workspace"]).assert_success("real").stderr_contains(
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
        .assert_success2("powerset_deduplication",  Some(if has_stable_toolchain() { 34 } else { 41 }))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on deduplication (1/9)
            running `cargo check --no-default-features --features a` on deduplication (2/9)
            running `cargo check --no-default-features --features b` on deduplication (3/9)
            running `cargo check --no-default-features --features c` on deduplication (4/9)
            running `cargo check --no-default-features --features d` on deduplication (5/9)
            running `cargo check --no-default-features --features e` on deduplication (6/9)
            running `cargo check --no-default-features --features easytime/default` on deduplication (7/9)
            running `cargo check --no-default-features --features easytime/std` on deduplication (8/9)
            running `cargo check --no-default-features --all-features` on deduplication (9/9)
            ",
        );
}

#[test]
fn trailing_args() {
    cargo_hack(["test", "--", "--ignored"])
        .assert_success("real")
        .stderr_contains("running `cargo test -- --ignored` on real")
        .stdout_contains(
            "
            running 1 test
            test tests::test_ignored
            ",
        );
}

#[test]
fn package_collision() {
    cargo_hack(["check"]).assert_success("package_collision").stderr_contains(
        "
        running `cargo check` on member1
        running `cargo check` on member2
        ",
    );
}

#[test]
fn not_find_manifest() {
    cargo_hack(["check"])
        .assert_success("virtual/dir/not_find_manifest")
        .stderr_not_contains(
            "
            running `cargo check` on member1
            running `cargo check` on member2
            ",
        )
        .stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack(["check", "--all"]).assert_success("virtual/dir/not_find_manifest").stderr_contains(
        "
        running `cargo check` on member1
        running `cargo check` on member2
        running `cargo check` on not_find_manifest
        ",
    );

    cargo_hack(["check", "--manifest-path", "dir/not_find_manifest/Cargo.toml"])
        .assert_success("virtual")
        .stderr_not_contains(
            "
            running `cargo check` on member1
            running `cargo check` on member2
            ",
        )
        .stderr_contains("running `cargo check` on not_find_manifest");

    cargo_hack(["check", "--all", "--manifest-path", "dir/not_find_manifest/Cargo.toml"])
        .assert_success("virtual")
        .stderr_contains(
            "
            running `cargo check` on member1
            running `cargo check` on member2
            running `cargo check` on not_find_manifest
            ",
        );
}

#[test]
fn optional_deps() {
    // require Rust 1.31 due to optional_deps uses renamed deps
    cargo_hack(["run", "--features=real,member2,renemed", "--ignore-unknown-features"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains(
            "
            skipped applying unknown `member2` feature to optional_deps
            running `cargo run --features real,renemed` on optional_deps
            ",
        )
        .stdout_contains(
            "
            renemed
            real
            ",
        )
        .stdout_not_contains(
            "
            member3
            member2
            ",
        );

    cargo_hack(["check", "--each-feature"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/2)
            running `cargo check --no-default-features --all-features` on optional_deps (2/2)
            ",
        )
        .stderr_not_contains(
            "
            --features real
            --features renemed
            ",
        );

    cargo_hack(["check", "--each-feature", "--optional-deps"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/4)
            running `cargo check --no-default-features --features real` on optional_deps (2/4)
            running `cargo check --no-default-features --features renemed` on optional_deps (3/4)
            running `cargo check --no-default-features --all-features` on optional_deps (4/4)
            ",
        );

    cargo_hack(["check", "--each-feature", "--optional-deps", "real"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/3)
            running `cargo check --no-default-features --features real` on optional_deps (2/3)
            running `cargo check --no-default-features --all-features` on optional_deps (3/3)
            ",
        )
        .stderr_not_contains("--features renemed");

    cargo_hack(["check", "--each-feature", "--optional-deps=renemed"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/3)
            running `cargo check --no-default-features --features renemed` on optional_deps (2/3)
            running `cargo check --no-default-features --all-features` on optional_deps (3/3)
            ",
        )
        .stderr_not_contains("--features real");

    cargo_hack(["check", "--each-feature", "--optional-deps="])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/2)
            running `cargo check --no-default-features --all-features` on optional_deps (2/2)
            ",
        );
}

#[test]
fn optional_deps_failure() {
    cargo_hack(["check", "--optional-deps"])
        .assert_failure("real")
        .stderr_contains(
            "--optional-deps can only be used together with either --each-feature or --feature-powerset",
        );
}

#[test]
fn skip_optional_deps() {
    // require Rust 1.31 due to optional_deps uses renamed deps
    cargo_hack(["check", "--each-feature", "--optional-deps", "--exclude-features", "real"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on optional_deps (1/2)
            running `cargo check --no-default-features --features renemed` on optional_deps (2/2)
            ",
        )
        .stderr_not_contains("--features real");
}

#[test]
fn list_separator() {
    // require Rust 1.31 due to optional_deps uses renamed deps
    cargo_hack(["run", "--features='real,renemed'"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features=\"real,renemed\""])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features=real,renemed"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features", "real,renemed"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features='real renemed'"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features=\"real renemed\""])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains("running `cargo run --features real,renemed` on optional_deps");

    cargo_hack(["run", "--features", "real renemed"])
        .assert_success2("optional_deps", Some(31))
        .stderr_contains("running `cargo run --features real,renemed` on optional_deps");
}

#[test]
fn short_flag() {
    cargo_hack(["check", "-vvpmember1"]) // same as -v -v -p member1
        .assert_success("virtual")
        .stderr_contains(format!(
            "
            cargo{EXE_SUFFIX} check -v --manifest-path member1{MAIN_SEPARATOR}Cargo.toml` (1/1)
            ",
        ))
        .stderr_not_contains("member2");

    cargo_hack(["check", "-qpmember1"]) // same as -q -p member1
        .assert_success("virtual")
        .stderr_contains("`cargo check -q` on member1 (1/1)")
        .stderr_not_contains("member2");
}

#[test]
fn verbose() {
    cargo_hack(["check", "--verbose"]).assert_success("virtual").stderr_contains(format!(
        "
        cargo{EXE_SUFFIX} check --manifest-path member1{MAIN_SEPARATOR}Cargo.toml` (1/3)
        cargo{EXE_SUFFIX} check --manifest-path member2{MAIN_SEPARATOR}Cargo.toml` (2/3)
        cargo{EXE_SUFFIX} check --manifest-path dir{MAIN_SEPARATOR}not_find_manifest{MAIN_SEPARATOR}Cargo.toml` (3/3)
        ",
    ));

    // If `-vv` is passed, propagate `-v` to cargo.
    cargo_hack(["check", "-vv", "-p", "member1"]).assert_success("virtual").stderr_contains(
        format!(
            "
            cargo{EXE_SUFFIX} check -v --manifest-path member1{MAIN_SEPARATOR}Cargo.toml` (1/1)
            ",
        ),
    );
    cargo_hack(["check", "-vvv", "-p", "member1"]).assert_success("virtual").stderr_contains(
        format!(
            "
            cargo{EXE_SUFFIX} check -vv --manifest-path member1{MAIN_SEPARATOR}Cargo.toml` (1/1)
            ",
        ),
    );
}

#[test]
fn propagate() {
    // --features
    cargo_hack(["check", "--features", "a"]).assert_success("real").stderr_contains("--features a");
    cargo_hack(["check", "--features=a"]).assert_success("real").stderr_contains("--features a");

    // --no-default-features
    cargo_hack(["check", "--no-default-features"])
        .assert_success("real")
        .stderr_contains("--no-default-features");

    // --all-features
    cargo_hack(["check", "--all-features"])
        .assert_success("real")
        .stderr_contains("--all-features");

    // --color
    cargo_hack(["check", "--color", "auto"])
        .assert_success("real")
        .stderr_contains("`cargo check --color auto`");
    cargo_hack(["check", "--color=auto"])
        .assert_success("real")
        .stderr_contains("`cargo check --color auto`");

    // --target
    cargo_hack(["check", "--target", TARGET])
        .assert_success("real")
        .stderr_contains(format!("`cargo check --target {TARGET}`"));

    // --verbose does not be propagated
    cargo_hack(["check", "--verbose"]).assert_success("real").stderr_not_contains("--verbose");
}

#[test]
fn default_feature_behavior() {
    cargo_hack(["run"])
        .assert_success("default_feature_behavior/has_default")
        .stdout_contains("has default feature!")
        .stdout_not_contains("no default feature!");

    cargo_hack(["run", "--no-default-features"])
        .assert_success("default_feature_behavior/has_default")
        .stdout_contains("no default feature!")
        .stdout_not_contains("has default feature!");

    cargo_hack(["run"])
        .assert_success("default_feature_behavior/no_default")
        .stdout_contains("no default feature!")
        .stdout_not_contains("has default feature!");

    cargo_hack(["run", "--no-default-features"])
        .assert_success("default_feature_behavior/no_default")
        .stdout_contains("no default feature!")
        .stdout_not_contains("has default feature!");
}

#[cfg_attr(windows, ignore)] // rustup bug: https://github.com/rust-lang/rustup/issues/3036
#[test]
fn version_range() {
    cargo_hack(["check", "--version-range", "1.63..1.64"]).assert_success("real").stderr_contains(
        "
        running `cargo +1.63 check` on real (1/2)
        running `cargo +1.64 check` on real (2/2)
        ",
    );

    cargo_hack(["check", "--version-range", "1.63..1.64", "--target", TARGET])
        .assert_success("real")
        .stderr_contains(format!(
            "
            running `cargo +1.63 check --target {TARGET}` on real (1/2)
            running `cargo +1.64 check --target {TARGET}` on real (2/2)
            ",
        ));
}

#[cfg_attr(windows, ignore)] // rustup bug: https://github.com/rust-lang/rustup/issues/3036
#[test]
fn multitarget() {
    let target_suffix = if cfg!(target_os = "linux") && cfg!(target_env = "gnu") {
        "-unknown-linux-gnu"
    } else if cfg!(target_os = "macos") {
        "-apple-darwin"
    } else {
        unimplemented!()
    };

    cargo_hack([
        "check",
        "--version-range",
        "1.63..1.64",
        "--target",
        &format!("aarch64{target_suffix}"),
    ])
    .assert_success("real")
    .stderr_contains(format!(
        "
        running `cargo +1.63 check --target aarch64{target_suffix}` on real (1/2)
        running `cargo +1.64 check --target aarch64{target_suffix}` on real (2/2)
        "
    ));

    cargo_hack([
        "check",
        "--version-range",
        "1.63..1.64",
        "--target",
        &format!("x86_64{target_suffix}"),
        "--target",
        &format!("aarch64{target_suffix}"),
    ])
    .assert_success("real")
    .stderr_contains(format!(
        "
        running `cargo +1.63 check --target x86_64{target_suffix}` on real (1/3)
        running `cargo +1.63 check --target aarch64{target_suffix}` on real (2/3)
        running `cargo +1.64 check --target x86_64{target_suffix} --target aarch64{target_suffix}` on real (3/3)
        ",
    ));

    cargo_hack([
        "check",
        "--version-range",
        "1.63..1.64",
        "--target",
        &format!("x86_64{target_suffix}"),
        "--target",
        &format!("x86_64{target_suffix}"),
    ])
    .assert_success("real")
    .stderr_contains(format!(
        "
        running `cargo +1.63 check --target x86_64{target_suffix}` on real (1/2)
        running `cargo +1.64 check --target x86_64{target_suffix}` on real (2/2)
        ",
    ));
}

#[cfg_attr(windows, ignore)] // rustup bug: https://github.com/rust-lang/rustup/issues/3036
#[test]
fn version_range_failure() {
    // zero step
    cargo_hack(["check", "--version-range", "1.45..", "--version-step", "0"])
        .assert_failure("real")
        .stderr_contains("--version-step cannot be zero");

    // empty
    cargo_hack(["check", "--version-range", "1.45..1.44"])
        .assert_failure("real")
        .stderr_contains("specified version range `1.45..1.44` is empty");

    // v0
    cargo_hack(["check", "--version-range", "0.45.."])
        .assert_failure("real")
        .stderr_contains("major version must be 1");

    // patch version
    cargo_hack(["check", "--version-range", "1.45.2.."])
        .assert_failure("real") // warn
        .stderr_contains(
            "
            --version-range always selects the latest patch release per minor release, \
            not the specified patch release `2`
            ",
        );
}

#[test]
fn clean_per_version_failure() {
    if env::var_os("CARGO_HACK_TEST_TOOLCHAIN").is_some() {
        return;
    }

    // without --version-range
    cargo_hack(["check", "--clean-per-version"])
        .assert_failure("real")
        .stderr_contains("--clean-per-version can only be used together with --version-range");
}

#[test]
fn keep_going() {
    cargo_hack(["check", "--each-feature", "--keep-going"])
        .assert_failure("keep_going")
        .stderr_contains(format!(
            "
            running `cargo check --no-default-features` on keep_going (1/2)
            `a` feature not specified
            running `cargo check --no-default-features --features a` on keep_going (2/2)
            `a` feature specified
            failed to run 2 commands
            failed commands:
            keep_going:
            cargo{EXE_SUFFIX} check --manifest-path Cargo.toml --no-default-features`
            cargo{EXE_SUFFIX} check --manifest-path Cargo.toml --no-default-features --features a`
            ",
        ));
}

#[test]
fn namespaced_features() {
    cargo_hack(["check", "--feature-powerset"])
        .assert_success2("namespaced_features", Some(60))
        .stderr_contains(
            "
            running `cargo check --no-default-features` on namespaced_features (1/2)
            running `cargo check --no-default-features --features easytime` on namespaced_features (2/2)
            ",
        );
}

#[test]
fn empty_string() {
    cargo_hack(["check", "--each-feature", "--skip", ""])
        .assert_success("real")
        .stderr_not_contains("not found");
}
