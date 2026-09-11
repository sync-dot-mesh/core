//! Ports: traits the engine depends on but does not implement.
//! Concrete implementations ("adapters") live in `sync-mesh-daemon`
//! for production, or as fakes in tests. Adding a new port here and
//! an adapter in daemon is how new infrastructure (a different
//! transport, a different change-detection backend) plugs in later
//! without engine code changing.

mod clock;

pub use clock::Clock;
