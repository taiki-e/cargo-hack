use anyhow::Context;
use std::{
    fs, mem,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    cli::{Args, Coloring},
    manifest::Manifest,
    Result,
};

#[derive(Clone)]
pub(crate) struct Restore {
    color: Option<Coloring>,
    restore_flag: bool,

    current: Arc<Mutex<Option<Current>>>,
}

struct Current {
    manifest: String,
    manifest_path: PathBuf,
    // A flag that indicates a restore is needed.
    restore_flag: bool,
}

impl Restore {
    pub(crate) fn new(args: &Args) -> Self {
        let this = Self {
            color: args.color,
            // if `--remove-dev-deps` flag is off, restore manifest file.
            restore_flag: args.no_dev_deps && !args.remove_dev_deps,
            current: Arc::new(Mutex::new(None)),
        };

        let x = this.clone();
        ctrlc::set_handler(move || {
            if let Err(e) = x.restore_dev_deps() {
                error!(x.color, "{:#}", e);
                std::process::exit(1)
            }
            std::process::exit(0)
        })
        .unwrap();

        this
    }

    pub(crate) fn set_manifest(&self, manifest: &Manifest) -> Handle<'_> {
        *self.current.lock().unwrap() = Some(Current {
            manifest: manifest.raw.to_string(),
            manifest_path: manifest.path.to_path_buf(),
            restore_flag: self.restore_flag,
        });

        Handle(Some(self))
    }

    fn restore_dev_deps(&self) -> Result<()> {
        let mut current = self.current.lock().unwrap();
        if let Some(current) = &mut *current {
            if mem::replace(&mut current.restore_flag, false) {
                fs::write(&current.manifest_path, &current.manifest).with_context(|| {
                    format!("failed to restore manifest file: {}", current.manifest_path.display())
                })?;
            }
        }
        Ok(())
    }
}

pub(crate) struct Handle<'a>(Option<&'a Restore>);

impl Handle<'_> {
    pub(crate) fn done(&mut self) -> Result<()> {
        self.0.take().unwrap().restore_dev_deps()
    }
}

impl Drop for Handle<'_> {
    fn drop(&mut self) {
        if let Some(this) = self.0 {
            if let Err(e) = this.restore_dev_deps() {
                error!(this.color, "{:#}", e);
            }
        }
    }
}
