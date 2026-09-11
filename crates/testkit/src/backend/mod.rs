pub mod process;

use std::path::PathBuf;

use uuid::Uuid;

/// How [`TestCluster`](crate::TestCluster) spawns, reaches, and stops
/// one test instance. `TestCluster<B>` is generic over this trait
/// rather than boxing it as `dyn` — a test picks its backend
/// explicitly and pays no dynamic-dispatch cost for it. Every test
/// body written against this trait works unchanged against any future
/// backend that implements it; only this trait's implementors need to
/// know anything about Docker, SSH, or ADB.
pub trait ClusterBackend: Send + Sync {
    /// Whatever a spawned instance's backend needs to remember about
    /// it — a child process handle for `process`, a container ID for
    /// `docker`, and so on. Backend-specific; nothing outside the
    /// backend inspects this type's fields.
    type Handle: Send;

    /// Starts one instance under this backend.
    ///
    /// # Errors
    /// Returns an error if the instance could not be started at all —
    /// for the process backend, this means the daemon binary itself
    /// failed to launch (not found, not executable). It does not mean
    /// the instance is reachable yet; use [`grpc_endpoint`](Self::grpc_endpoint)
    /// to wait for that.
    fn spawn(
        &self,
        config: &NodeConfig,
    ) -> impl std::future::Future<Output = Result<Self::Handle, BackendError>> + Send;

    /// Waits for and returns a connectable endpoint for an already-
    /// spawned instance.
    ///
    /// # Errors
    /// Returns [`BackendError::HandshakeTimeout`] if the instance never
    /// becomes reachable within the backend's timeout — this is the
    /// expected outcome for an instance that failed to start cleanly
    /// (e.g. the negative lifecycle test), not necessarily a bug.
    fn grpc_endpoint(
        &self,
        handle: &Self::Handle,
    ) -> impl std::future::Future<Output = Result<String, BackendError>> + Send;

    /// Stops an already-spawned instance.
    ///
    /// # Errors
    /// Returns an error if the backend could not confirm the instance
    /// stopped — for the process backend this is effectively
    /// best-effort and rarely fails in practice, since killing a child
    /// process is not expected to fail under normal conditions.
    fn stop(
        &self,
        handle: Self::Handle,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;
}

/// What a backend needs to spawn one node. Deliberately minimal today —
/// grows as later tests need more (a peer list to pre-seed for
/// discovery tests, a `SyncIntensity` override, ...), each addition
/// justified by the test that actually needs it.
pub struct NodeConfig {
    pub data_dir: PathBuf,
    /// Threaded through to the spawned instance for the mDNS-isolation
    /// scheme the discovery tests will need — see the daemon's
    /// `--test-run-id` flag. Not used by anything yet.
    pub test_run_id: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("failed to spawn instance: {0}")]
    Spawn(String),

    #[error("instance never became reachable within the timeout")]
    HandshakeTimeout,

    #[error("failed to parse handshake data: {0}")]
    HandshakeParse(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
