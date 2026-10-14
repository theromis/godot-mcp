use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks a mutex, recovering the data if another thread panicked while holding it.
///
/// The guarded maps in this crate stay structurally valid after a panic, so continuing is
/// safer than propagating the poison and taking the whole server down.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
