use std::collections::HashMap;
use std::path::PathBuf;

use sync_mesh_proto::sync_service_client::SyncServiceClient;
use tonic::transport::Channel;
use uuid::Uuid;

use crate::backend::{BackendError, ClusterBackend, NodeConfig};

/// Orchestrates N test instances against whichever [`ClusterBackend`]
/// it's parameterised with. Every test writes against this type once;
/// the backend chosen at construction is the only thing that changes
/// between "runs as local processes" and, later, "runs as containers"
/// or "runs on real Android hardware."
///
/// All spawned instances share one `test_run_id`, generated once per
/// cluster — this is what the mDNS-based discovery isolation (planned,
/// not yet built) will filter on, so that two test functions running
/// concurrently under `cargo test`'s default parallelism can never
/// cross-discover each other's instances.
pub struct TestCluster<B: ClusterBackend> {
    backend: B,
    run_id: Uuid,
    nodes: HashMap<String, B::Handle>,
    // Held only so the directory and everything under it is removed
    // when the cluster is dropped — never read directly.
    _temp_root: tempfile::TempDir,
}

impl<B: ClusterBackend> TestCluster<B> {
    pub fn new(backend: B) -> std::io::Result<Self> {
        Ok(Self {
            backend,
            run_id: Uuid::new_v4(),
            nodes: HashMap::new(),
            _temp_root: tempfile::tempdir()?,
        })
    }

    /// The identifier shared by every instance in this cluster.
    /// Exposed so a test can build a second cluster deliberately
    /// sharing (or deliberately not sharing) isolation scope, once
    /// discovery tests need that.
    pub fn run_id(&self) -> Uuid {
        self.run_id
    }

    fn data_dir_for(&self, name: &str) -> PathBuf {
        self._temp_root.path().join(name)
    }

    /// Spawns one node under this cluster's isolation scope. `name` is
    /// only a local label for later lookups (`status_client`,
    /// `stop_node`) — it is never sent anywhere.
    pub async fn spawn_node(&mut self, name: impl Into<String>) -> Result<(), BackendError> {
        let name = name.into();
        let config = NodeConfig { data_dir: self.data_dir_for(&name), test_run_id: self.run_id };

        let handle = self.backend.spawn(&config).await?;
        self.nodes.insert(name, handle);
        Ok(())
    }

    /// A connected gRPC client for the named node's `SyncService`.
    /// Waits for the instance to actually become reachable (via the
    /// backend's handshake) rather than assuming it's ready the
    /// instant `spawn_node` returns.
    pub async fn status_client(
        &self,
        name: &str,
    ) -> Result<SyncServiceClient<Channel>, BackendError> {
        let handle = self.nodes.get(name).ok_or_else(|| {
            BackendError::Spawn(format!("no node named {name:?} in this cluster"))
        })?;

        let endpoint = self.backend.grpc_endpoint(handle).await?;

        SyncServiceClient::connect(endpoint).await.map_err(|e| BackendError::Spawn(e.to_string()))
    }

    /// Explicitly stops one node without waiting for the whole
    /// cluster to be dropped — needed by tests that check behaviour
    /// *during* a partial cluster (e.g. the disruption test, later).
    pub async fn stop_node(&mut self, name: &str) -> Result<(), BackendError> {
        if let Some(handle) = self.nodes.remove(name) {
            self.backend.stop(handle).await?;
        }
        Ok(())
    }

    pub fn node_names(&self) -> impl Iterator<Item = &str> {
        self.nodes.keys().map(String::as_str)
    }
}
