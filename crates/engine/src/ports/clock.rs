use std::time::Instant;

/// A source of the current instant.
///
/// The engine never calls `Instant::now()` directly anywhere — it asks
/// a `Clock` instead. This is a small seam, but it is the one that is
/// expensive to retrofit if it is missing: once uptime, timeouts, and
/// (later) pairing-token expiry are all sprinkled with direct
/// `Instant::now()` calls, none of that logic can be tested without
/// actually waiting in real time. With the port in place from the
/// start, a test can hand in a fake clock it controls directly.
///
/// `Instant` (monotonic, immune to wall-clock adjustments) rather than
/// `SystemTime` — correct for "how long has this process run", which
/// is the only thing that needs it so far. Wall-clock timestamps (a
/// file's modification time, a pairing token's absolute expiry) are a
/// different need and would warrant a separate port when that code is
/// actually written, not a reason to make this one do double duty now.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}
