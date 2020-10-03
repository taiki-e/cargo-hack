#!/bin/bash

# Check all public crates with minimal version dependencies.
#
# Usage:
#    bash scripts/check-minimal-versions.sh
#
# Note: this script modifies Cargo.toml and Cargo.lock while this script is
# running, and it is an error if there are any unstaged changes.
#
# Refs: https://github.com/rust-lang/cargo/issues/5657

set -euo pipefail

cd "$(cd "$(dirname "${0}")" && pwd)"/..

if [[ "${CI:-false}" != "true" ]]; then
    toolchain="+nightly"
fi

# This script modifies Cargo.toml and Cargo.lock, so make sure there are no
# unstaged changes.
git diff --exit-code
# Restore original Cargo.toml and Cargo.lock on exit.
trap 'git checkout .' EXIT

# Remove dev-dependencies from Cargo.toml to prevent the next `cargo update`
# from determining minimal versions based on dev-dependencies.
cargo hack --remove-dev-deps --workspace

# Update Cargo.lock to minimal version dependencies.
cargo ${toolchain:-} update -Zminimal-versions
# Run check for all public members of the workspace.
cargo ${toolchain:-} hack check --workspace --all-features --ignore-private -Zfeatures=all
