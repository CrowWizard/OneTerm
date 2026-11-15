//! Shared real-child spawn coordination for native, session and orphan tests.

// Orphan tests diff this process's children around startup. Hold this lock for
// the whole native fixture so its PTY is never mistaken for a legacy orphan.
static SPAWN_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn lock_spawns() -> std::sync::MutexGuard<'static, ()> {
    SPAWN_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
