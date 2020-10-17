#![forbid(unsafe_code)]
#![warn(future_incompatible, rust_2018_idioms, single_use_lifetimes, unreachable_pub)]
#![warn(clippy::all, clippy::default_trait_access)]
// mem::take and #[non_exhaustive] requires Rust 1.40, matches! requires Rust 1.42
#![allow(
    clippy::mem_replace_with_default,
    clippy::manual_non_exhaustive,
    clippy::match_like_matches_macro
)]

#[macro_use]
mod term;

mod cli;
mod manifest;
mod metadata;
mod package;
mod process;
mod remove_dev_deps;
mod restore;
mod workspace;

use anyhow::Error;
use std::{env, ffi::OsString, path::Path};

use crate::{
    cli::{Args, Coloring},
    manifest::Manifest,
    metadata::Metadata,
    process::ProcessBuilder,
};

type Result<T, E = Error> = std::result::Result<T, E>;

fn main() {
    let mut coloring = None;
    if let Err(e) = try_main(&mut coloring) {
        error!(coloring, "{:#}", e);
        std::process::exit(1)
    }
}

fn try_main(coloring: &mut Option<Coloring>) -> Result<()> {
    let args = cli::RawArgs::new();
    let args = args.perse(coloring)?.unwrap_or_else(|| std::process::exit(0));
    let metadata = Metadata::new(&args)?;

    let current_manifest = match args.manifest_path {
        Some(path) => Manifest::new(Path::new(path))?,
        None => Manifest::new(manifest::find_root_manifest_for_wd(&env::current_dir()?)?)?,
    };

    // TODO: Ideally, we should do this, but for now, we allow it as cargo-hack
    // may mistakenly interpret the specified valid feature flag as unknown.
    // if args.ignore_unknown_features && !args.workspace && !current_manifest.is_virtual() {
    //     bail!(
    //         "--ignore-unknown-features can only be used in the root of a virtual workspace or together with --workspace"
    //     )
    // }

    workspace::exec(&args, &current_manifest, &metadata)
}

fn cargo_binary() -> OsString {
    env::var_os("CARGO_HACK_CARGO_SRC")
        .unwrap_or_else(|| env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")))
}
