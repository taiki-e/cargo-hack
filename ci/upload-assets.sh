#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

cd "$(cd "$(dirname "${0}")" && pwd)"/..

ref="${GITHUB_REF:?}"
tag="${ref#*/tags/}"

export CARGO_PROFILE_RELEASE_LTO=true
host=$(rustc -Vv | grep host | sed 's/host: //')

package="cargo-hack"
cargo build --bin "${package}" --release

cd target/release
case "${OSTYPE}" in
  linux* | darwin*)
    strip "${package}"
    asset="${package}-${host}.tar.gz"
    # TODO: remove this when release the next major version.
    asset2="${package}-${tag}-${host}.tar.gz"
    tar czf ../../"${asset}" "${package}"
    tar czf ../../"${asset2}" "${package}"
    ;;
  cygwin* | msys*)
    asset="${package}-${host}.zip"
    # TODO: remove this when release the next major version.
    asset2="${package}-${tag}-${host}.zip"
    7z a ../../"${asset}" "${package}".exe
    7z a ../../"${asset2}" "${package}".exe
    ;;
  *)
    echo "unrecognized OSTYPE: ${OSTYPE}"
    exit 1
    ;;
esac
cd ../..

if [[ -z "${GITHUB_TOKEN:-}" ]]; then
  echo "GITHUB_TOKEN not set, skipping deploy"
  exit 1
else
  gh release upload "${tag}" "${asset}" "${asset2}" --clobber
fi
