#!/bin/bash

# Check all public crates with minimal version dependencies.
#
# Usage:
#    bash scripts/check-minimal-versions.sh [+toolchain] [check|test]
#
# Note:
# - This script modifies Cargo.toml and Cargo.lock while running
# - This script exits with 1 if there are any unstaged changes
# - This script requires nightly Rust and cargo-hack
#
# Refs: https://github.com/rust-lang/cargo/issues/5657

set -euo pipefail

cd "$(cd "$(dirname "${0}")" && pwd)"/..

# Decide Rust toolchain.
# Nightly is used by default if the `CI` environment variable is not set to `true`.
if [[ "${1:-}" == "+"* ]]; then
    toolchain="${1}"
    shift
elif [[ "${CI:-}" != "true" ]]; then
    toolchain="+nightly"
fi
# Make sure toolchain is installed.
cargo ${toolchain:-} -V >/dev/null || exit 1
# This script requires nightly Rust and cargo-hack
if [[ "${toolchain:-+nightly}" != "+nightly"* ]] || ! cargo hack -V &>/dev/null; then
    echo "error: check-minimal-versions.sh requires nightly Rust and cargo-hack"
    exit 1
fi

# Parse subcommand.
subcmd="${1:-check}"
if [[ ! "${subcmd}" =~ check|test ]]; then
    echo "error: invalid argument \`${1}\`"
    exit 1
fi

# This script modifies Cargo.toml and Cargo.lock, so make sure there are no
# unstaged changes.
git diff --exit-code
# Restore original Cargo.toml and Cargo.lock on exit.
trap 'git checkout .' EXIT

if [[ "${subcmd}" == "check" ]]; then
    # Remove dev-dependencies from Cargo.toml to prevent the next `cargo update`
    # from determining minimal versions based on dev-dependencies.
    cargo hack --remove-dev-deps --workspace
fi

# Update Cargo.lock to minimal version dependencies.
cargo ${toolchain:-} update -Zminimal-versions
# Run check for all public members of the workspace.
cargo ${toolchain:-} hack "${subcmd}" --workspace --all-features --ignore-private -Zfeatures=all
