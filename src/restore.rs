use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;

use crate::{fs, term};

#[derive(Clone)]
pub(crate) struct Manager {
    // A flag that indicates restore is needed.
    pub(crate) needs_restore: bool,
    // Information on file that needs to be restored next.
    // If `needs_restore` is `false`, this is always `None`.
    current: Arc<Mutex<Option<File>>>,
}

impl Manager {
    pub(crate) fn new(needs_restore: bool) -> Self {
        let this = Self { needs_restore, current: Arc::new(Mutex::new(None)) };

        let cloned = this.clone();
        ctrlc::set_handler(move || {
            if let Err(e) = cloned.restore() {
                error!("{:#}", e);
                std::process::exit(1)
            }
            std::process::exit(0)
        })
        .unwrap();

        this
    }

    pub(crate) fn set(&self, text: impl Into<String>, path: impl Into<PathBuf>) -> Handle<'_> {
        if !self.needs_restore {
            return Handle(None);
        }

        *self.current.lock().unwrap() = Some(File { text: text.into(), path: path.into() });

        Handle(Some(self))
    }

    fn restore(&self) -> Result<()> {
        let mut current = self.current.lock().unwrap();
        if let Some(file) = current.take() {
            file.restore()?;
        }
        Ok(())
    }
}

struct File {
    /// The original text of this file.
    text: String,
    /// Path to this file.
    path: PathBuf,
}

impl File {
    fn restore(self) -> Result<()> {
        if term::verbose() {
            info!("restoring {}", self.path.display());
        }
        fs::write(&self.path, &self.text)
    }
}

#[must_use]
pub(crate) struct Handle<'a>(Option<&'a Manager>);

impl Handle<'_> {
    pub(crate) fn close(&mut self) -> Result<()> {
        if let Some(manager) = self.0.take() {
            manager.restore()?;
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
