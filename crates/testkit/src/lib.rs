//! Spawns and drives real Sync.Mesh instances for integration tests —
//! never mocked, never in-process shortcuts. The pattern this exists
//! to avoid: a test suite that only ever exercises code running in
//! the test's own process, which proves nothing about how the actual
//! compiled daemon binary behaves when it's a real separate process
//! with its own crash domain, its own port, its own filesystem state.
//!
//! [`backend::ClusterBackend`] is the seam for the tiers described in
//! the integration-testing backlog: `process` (implemented here, Tier
//! 0), `docker` (Tier 1), `remote` (Tier 2), `android` (Tier 3). Test
//! bodies are written once, against [`TestCluster`], and are generic
//! over which backend they run against — adding a later tier means
//! writing a new backend, not new tests.

pub mod backend;
mod cluster;

pub use backend::{process::ProcessBackend, BackendError, ClusterBackend, NodeConfig};
pub use cluster::TestCluster;
