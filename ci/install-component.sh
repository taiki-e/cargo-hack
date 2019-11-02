#!/bin/bash

set -euo pipefail

set +e
if rustup component add "${1}"; then
    set -e
else
    set -e
    # If the component is unavailable on the latest nightly,
    # use the latest toolchain with the component available.
    # Refs: https://github.com/rust-lang/rustup-components-history#the-web-part
    target=$(curl -sSf "https://rust-lang.github.io/rustup-components-history/x86_64-unknown-linux-gnu/${1}")
    echo "'${1}' is unavailable on the toolchain '${RUST_TOOLCHAIN:-nightly}', use the toolchain 'nightly-${target}' instead"

    rustup update "nightly-${target}" --no-self-update
    rustup default "nightly-${target}"

    echo "Query rust and cargo versions:"
    rustup -V
    rustc -V
    cargo -V

    rustup component add "${1}"
fi

echo "Query component versions:"
case "${1}" in
    clippy) cargo clippy -V ;;
    rustfmt) rustfmt -V ;;
esac
