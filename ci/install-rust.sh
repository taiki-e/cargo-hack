#!/bin/bash

set -euo pipefail

case "${AGENT_OS}" in
    macos-*)
        curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}"
        export PATH=${PATH}:${HOME}/.cargo/bin
        echo "##[add-path]${HOME}/.cargo/bin"
        ;;
    ubuntu-* | windows-*)
        rustup set profile minimal
        rustup update "${RUST_TOOLCHAIN}" --no-self-update
        rustup default "${RUST_TOOLCHAIN}"
        ;;
esac

echo "Query rust and cargo versions:"
rustup -V
rustc -V
cargo -V
