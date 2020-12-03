#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

PACKAGE="cargo-hack"

function error {
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    echo "::error::$*"
  else
    echo "error: $*" >&2
  fi
}

cd "$(cd "$(dirname "${0}")" && pwd)"/..

if [[ "${GITHUB_REF:?}" != "refs/tags/"* ]]; then
  error "GITHUB_REF should start with 'refs/tags/'"
  exit 1
fi
tag="${GITHUB_REF#refs/tags/}"

host=$(rustc -Vv | grep host | sed 's/host: //')
target="${1:-"${host}"}"
cargo="cargo"
if [[ "${host}" != "${target}" ]]; then
  cargo="cross"
  cargo install cross
fi

export CARGO_PROFILE_RELEASE_LTO=true

$cargo build --bin "${PACKAGE}" --release --target "${target}"

assets=("${PACKAGE}-${target}.tar.gz")
cd target/"${target}"/release
case "${OSTYPE}" in
  linux* | darwin*)
    strip "${PACKAGE}"
    tar czf ../../"${assets[0]}" "${PACKAGE}"
    # TODO: remove this when release the next major version.
    if [[ ${target} != "x86_64-unknown-linux-musl" ]]; then
      assets+=("${PACKAGE}-${tag}-${target}.tar.gz")
      tar czf ../../"${assets[1]}" "${PACKAGE}"
    fi
    ;;
  cygwin* | msys*)
    assets+=("${PACKAGE}-${target}.zip")
    tar czf ../../"${assets[0]}" "${PACKAGE}".exe
    7z a ../../"${assets[1]}" "${PACKAGE}".exe
    # TODO: remove this when release the next major version.
    assets+=("${PACKAGE}-${tag}-${target}.zip")
    7z a ../../"${assets[2]}" "${PACKAGE}".exe
    ;;
  *)
    error "unrecognized OSTYPE: ${OSTYPE}"
    exit 1
    ;;
esac
cd ../..

if [[ -z "${GITHUB_TOKEN:-}" ]]; then
  error "GITHUB_TOKEN not set, skipping deploy"
  exit 1
else
  gh release upload "${tag}" "${assets[@]}" --clobber
fi
