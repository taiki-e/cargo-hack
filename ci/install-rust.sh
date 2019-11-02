#!/bin/bash

set -euo pipefail

case "${AGENT_OS:-ubuntu-latest}" in
    macos-*)
        curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN:-nightly}"
        export PATH=${PATH}:${HOME}/.cargo/bin
        echo "##[add-path]${HOME}/.cargo/bin"
        ;;
    ubuntu-* | windows-*)
        rustup set profile minimal
        rustup update "${RUST_TOOLCHAIN:-nightly}" --no-self-update
        rustup default "${RUST_TOOLCHAIN:-nightly}"
        ;;
esac

echo "Query rust and cargo versions:"
rustup -V
rustc -V
cargo -V
