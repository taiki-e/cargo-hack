#!/bin/bash

set -euo pipefail

case "${AGENT_OS}" in
    macos-*)
        curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}"
        export PATH=${PATH}:${HOME}/.cargo/bin
        echo "##[add-path]${HOME}/.cargo/bin"
        ;;
    ubuntu-* | windows-*)
        # TODO: when default rustup is bumped to 1.20+, enable this.
        # rustup set profile minimal
        rustup toolchain install "${RUST_TOOLCHAIN}" --no-self-update
        rustup default "${RUST_TOOLCHAIN}"
        ;;
esac

echo "Query rust and cargo versions:"
rustup -V
rustc -V
cargo -V
