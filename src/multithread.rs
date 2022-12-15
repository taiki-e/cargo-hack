use std::sync::{self, Arc, Mutex};

use nanoid::nanoid;

/// TargetDir pool
#[derive(Default)]
pub(crate) struct TargetDirPool {
    ready: Arc<Mutex<Vec<String>>>,
}

impl TargetDirPool {
    pub(crate) fn new() -> Self {
        Default::default()
    }

    pub(crate) fn get(&self) -> String {
        let mut ready = unpoison_mutex(self.ready.lock());

        let value = ready.pop().unwrap_or(nanoid!());
        value
    }
    pub(crate) fn give_back(&self, dir_name: String) {
        let mut ready = unpoison_mutex(self.ready.lock());
        ready.push(dir_name)
    }
}

pub(crate) fn unpoison_mutex<T>(
    lock: sync::LockResult<sync::MutexGuard<'_, T>>,
) -> sync::MutexGuard<'_, T> {
    match lock {
        Ok(res) => res,
        Err(eres) => {
            let res_inner = eres.into_inner();
            eprintln!("ERROR: Mutex Lock poisoned");
            res_inner
        }
    }
}
