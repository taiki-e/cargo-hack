use std::sync::{self, Arc, Mutex};

use nanoid::nanoid;

/// ``TargetDir`` pool
///
/// Provides access to parallel build directories
#[derive(Default)]
pub(crate) struct TargetDirPool {
    ready: Arc<Mutex<Vec<String>>>,
}

impl TargetDirPool {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get(&self) -> String {
        let mut ready = cure_mutex(self.ready.lock());

        ready.pop().unwrap_or_else(|| nanoid!())
    }
    pub(crate) fn give_back(&self, dir_name: String) {
        let mut ready = cure_mutex(self.ready.lock());
        ready.push(dir_name);
    }
}

pub(crate) fn cure_mutex<T>(
    lock: sync::LockResult<sync::MutexGuard<'_, T>>,
) -> sync::MutexGuard<'_, T> {
    match lock {
        Ok(res) => res,
        Err(error_response) => {
            eprintln!("ERROR: Mutex Lock poisoned");
            error_response.into_inner()
        }
    }
}
