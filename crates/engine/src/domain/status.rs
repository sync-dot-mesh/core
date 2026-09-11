use std::time::Duration;

use super::NodeId;

/// What a daemon instance's status actually is.
///
/// This is deliberately an enum, not a `bool`. A bare `is_healthy:
/// bool` can only ever say yes or no — it cannot distinguish "still
/// starting up, ask again shortly" from "actually broken" from
/// "healthy". A caller (or a test) that only sees `false` has no way
/// to tell those apart without inventing a second field, and nothing
/// stops that second field from disagreeing with the first (`healthy:
/// false, starting: false, broken: false` — a state that means
/// nothing). The enum makes the actually-possible states the only
/// representable ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    /// Accepting connections is not yet safe — still initialising.
    Starting,
    /// Fully operational.
    Healthy,
    /// Shutdown has been requested; finishing in-flight work before
    /// exiting. Still respond to status checks, just say so honestly.
    ShuttingDown,
}

/// The full answer to "how is this instance doing" — what the
/// `GetStatus` RPC reports, expressed in domain terms rather than the
/// wire (protobuf) representation. Translating this into the proto
/// `GetStatusResponse` is the gRPC adapter's job, not this crate's —
/// this type has no idea gRPC exists.
#[derive(Debug, Clone, Copy)]
pub struct DaemonInfo {
    pub node_id: NodeId,
    pub health: HealthState,
    pub uptime: Duration,
}
