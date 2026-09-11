use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tonic::{Request, Response, Status};

use sync_mesh_engine::domain::{HealthState as EngineHealth, NodeId};
use sync_mesh_engine::ports::Clock;
use sync_mesh_proto::sync_service_server::SyncService;
use sync_mesh_proto::{GetStatusRequest, GetStatusResponse, HealthState as ProtoHealth};

/// The gRPC adapter for the engine's status concept. This is the one
/// place that is allowed to know both the engine's [`EngineHealth`]
/// and the wire format's `ProtoHealth` — translating between the two
/// is the entire point of this type. The engine crate never imports
/// `tonic` or anything generated from the `.proto`; this is where
/// domain language and wire language meet.
///
/// Internally `Arc`-backed and cheaply `Clone` — one clone is handed
/// to tonic's server (which takes ownership of whatever implements
/// `SyncService`), another stays in `main` so it can call
/// `mark_healthy`/`mark_shutting_down` on the *same* shared state the
/// server is actually answering `GetStatus` from. Wrapping the whole
/// service in an external `Arc<StatusService>` instead would hit
/// Rust's orphan rule the moment it needs to implement the
/// proto-generated `SyncService` trait — `Arc` isn't local to this
/// crate. Keeping the sharing internal avoids that entirely.
#[derive(Clone)]
pub struct StatusService {
    inner: Arc<Inner>,
}

struct Inner {
    node_id: NodeId,
    clock: Arc<dyn Clock>,
    started_at: Instant,
    // Three variants, stored as a small integer behind an atomic so
    // health can be updated concurrently from shutdown handling
    // without a lock. The encoding is private to this file.
    health: AtomicU8,
}

const HEALTH_STARTING: u8 = 0;
const HEALTH_HEALTHY: u8 = 1;
const HEALTH_SHUTTING_DOWN: u8 = 2;

impl StatusService {
    pub fn new(node_id: NodeId, clock: Arc<dyn Clock>) -> Self {
        let started_at = clock.now();
        Self {
            inner: Arc::new(Inner {
                node_id,
                clock,
                started_at,
                health: AtomicU8::new(HEALTH_STARTING),
            }),
        }
    }

    pub fn mark_healthy(&self) {
        self.inner.health.store(HEALTH_HEALTHY, Ordering::SeqCst);
    }

    pub fn mark_shutting_down(&self) {
        self.inner.health.store(HEALTH_SHUTTING_DOWN, Ordering::SeqCst);
    }

    fn current_health(&self) -> EngineHealth {
        match self.inner.health.load(Ordering::SeqCst) {
            HEALTH_HEALTHY => EngineHealth::Healthy,
            HEALTH_SHUTTING_DOWN => EngineHealth::ShuttingDown,
            _ => EngineHealth::Starting,
        }
    }
}

fn to_proto_health(health: EngineHealth) -> ProtoHealth {
    match health {
        EngineHealth::Starting => ProtoHealth::Starting,
        EngineHealth::Healthy => ProtoHealth::Healthy,
        EngineHealth::ShuttingDown => ProtoHealth::ShuttingDown,
    }
}

#[tonic::async_trait]
impl SyncService for StatusService {
    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        let uptime = self.inner.clock.now().duration_since(self.inner.started_at);
        let health = to_proto_health(self.current_health());

        Ok(Response::new(GetStatusResponse {
            node_id: self.inner.node_id.to_string(),
            health: health as i32,
            uptime_seconds: uptime.as_secs(),
        }))
    }
}
