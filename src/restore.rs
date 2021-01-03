use anyhow::{Context as _, Result};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{Context, PackageId};

#[derive(Clone)]
pub(crate) struct Restore {
    // A flag that indicates restore is needed.
    needs_restore: bool,
    // Information on manifest that needs to be restored next.
    // If `needs_restore` is `false`, this is always `None`.
    current: Arc<Mutex<Option<Current>>>,
}

struct Current {
    manifest: String,
    manifest_path: PathBuf,
}

impl Restore {
    pub(crate) fn new(cx: &Context<'_>) -> Self {
        let this = Self {
            // if `--remove-dev-deps` flag is off, restore manifest file.
            needs_restore: cx.no_dev_deps && !cx.remove_dev_deps,
            current: Arc::new(Mutex::new(None)),
        };

        if !this.needs_restore {
            return this;
        }

        let x = this.clone();
        ctrlc::set_handler(move || {
            if let Err(e) = x.restore_dev_deps() {
                error!("{:#}", e);
                std::process::exit(1)
            }
            std::process::exit(0)
        })
        .unwrap();

        this
    }

    pub(crate) fn set_manifest(&self, cx: &Context<'_>, id: &PackageId) -> Handle<'_> {
        if !self.needs_restore {
            return Handle(None);
        }

        *self.current.lock().unwrap() = Some(Current {
            manifest: cx.manifests(id).raw.clone(),
            manifest_path: cx.packages(id).manifest_path.clone(),
        });

        Handle(Some(self))
    }

    fn restore_dev_deps(&self) -> Result<()> {
        let mut current = self.current.lock().unwrap();
        if let Some(current) = current.take() {
            fs::write(&current.manifest_path, &current.manifest).with_context(|| {
                format!("failed to restore manifest file `{}`", current.manifest_path.display())
            })?;
        }
        Ok(())
    }
}

pub(crate) struct Handle<'a>(Option<&'a Restore>);

impl Handle<'_> {
    pub(crate) fn close(&mut self) -> Result<()> {
        if let Some(this) = self.0.take() {
            this.restore_dev_deps()?;
        }
        Ok(())
    }
}

impl Drop for Handle<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.close() {
            error!("{:#}", e);
        }
    }
}
