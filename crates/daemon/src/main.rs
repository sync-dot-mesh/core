//! The daemon binary: the composition root. This is the one place
//! allowed to know about every concrete adapter and wire them to the
//! engine's ports — nowhere else in the codebase should construct a
//! `SystemClock` or bind a real TCP listener directly.

mod data_dir;
mod grpc;
mod system_clock;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tonic::transport::Server;

use sync_mesh_engine::domain::NodeId;
use sync_mesh_proto::sync_service_server::SyncServiceServer;

use data_dir::DataDir;
use grpc::StatusService;
use system_clock::SystemClock;

#[derive(Parser, Debug)]
#[command(name = "sync-mesh-daemon")]
struct Args {
    /// Where this instance keeps its state. Only one daemon may run
    /// against a given data directory at a time — a second instance
    /// pointed here fails to start rather than racing the first.
    #[arg(long)]
    data_dir: PathBuf,

    /// Address to bind the gRPC server on. Defaults to an
    /// OS-assigned port on loopback — deliberate, not a placeholder:
    /// fixed ports flake under concurrent test runs, so tests never
    /// ask for one.
    #[arg(long, default_value = "127.0.0.1:0")]
    bind: SocketAddr,

    /// Tags this instance with a test-run identifier. Not used by
    /// anything yet — reserved for the mDNS-based discovery isolation
    /// planned for the next milestone (see the integration-testing
    /// backlog), threaded through now so the daemon's CLI surface and
    /// every test call site don't need to change again when discovery
    /// lands.
    #[arg(long)]
    test_run_id: Option<uuid::Uuid>,
}

/// Written into the data directory the instant the gRPC listener is
/// bound — how anything outside this process (the test harness, in
/// particular) learns which port the OS actually assigned. A file
/// rather than a stdout line: once real logging exists, daemon log
/// output will share stdout, and a reader would have to distinguish
/// "the handshake line" from "a log line that happens to look like
/// one." A dedicated file has no such ambiguity.
#[derive(serde::Serialize)]
struct HandshakeInfo {
    grpc_port: u16,
    pid: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let _ = args.test_run_id; // reserved; see the field's own doc comment

    // Fails cleanly here — not by corrupting shared state — if another
    // instance already holds this directory. This *is* the negative
    // case the lifecycle test exercises.
    let data_dir = DataDir::acquire(&args.data_dir)?;

    let node_id = NodeId::new();
    let clock = Arc::new(SystemClock);
    let status_service = StatusService::new(node_id, clock);

    let listener = tokio::net::TcpListener::bind(args.bind).await?;
    let actual_addr = listener.local_addr()?;

    let handshake = HandshakeInfo { grpc_port: actual_addr.port(), pid: std::process::id() };
    std::fs::write(data_dir.path().join("daemon.json"), serde_json::to_vec(&handshake)?)?;

    // Only now — after the port is bound and the handshake file is
    // written — is this instance actually ready to be considered up.
    status_service.mark_healthy();

    let shutdown_service = status_service.clone();
    let shutdown = async move {
        let _ = tokio::signal::ctrl_c().await;
        shutdown_service.mark_shutting_down();
    };

    Server::builder()
        .add_service(SyncServiceServer::new(status_service))
        .serve_with_incoming_shutdown(
            tokio_stream::wrappers::TcpListenerStream::new(listener),
            shutdown,
        )
        .await?;

    Ok(())
}
