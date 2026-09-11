use std::time::Instant;

use sync_mesh_engine::ports::Clock;

/// The real adapter for the engine's [`Clock`] port. Everything about
/// this is intentionally trivial — the value of the port is entirely
/// in *not* calling `Instant::now()` from engine code directly, so
/// that a test can substitute a fake clock. This is what the real
/// process actually uses.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
