use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::time::Instant;

use sync_mesh_handshake::HandshakeInfo;

use super::{BackendError, ClusterBackend, NodeConfig};

/// Spawns the compiled daemon binary as a real, separate OS process —
/// Tier 0 from the integration-testing backlog. This is deliberately
/// the *only* backend that exists so far; `docker`/`remote`/`android`
/// get added as separate modules implementing the same
/// [`ClusterBackend`] trait when those tiers are actually built.
pub struct ProcessBackend {
    daemon_binary: PathBuf,
}

impl ProcessBackend {
    /// `daemon_binary` is supplied by the caller rather than guessed —
    /// typically `env!("CARGO_BIN_EXE_sync-mesh-daemon")` from the
    /// test that actually lives in the daemon crate, since that macro
    /// only resolves correctly from within the crate that owns the
    /// binary, not from `testkit` itself.
    pub fn new(daemon_binary: impl Into<PathBuf>) -> Self {
        Self { daemon_binary: daemon_binary.into() }
    }
}

pub struct ProcessHandle {
    child: Child,
    data_dir: PathBuf,
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // Best-effort, synchronous-as-possible cleanup: a panicking
        // assertion elsewhere in a test must not leak this process.
        // `start_kill` is the non-blocking half of tokio's Child::kill
        // and is safe to call from a sync Drop; if the process is
        // already gone, this is a harmless no-op.
        let _ = self.child.start_kill();
    }
}

// This implementation never actually awaits anything in `spawn` —
// spawning a local process is synchronous. The trait method is async
// anyway, because the `docker`/`remote`/`android` backends this trait
// exists for genuinely will await real I/O (a container API call, an
// SSH connection, ADB). Changing the trait's signature to suit this
// one implementation would defeat the point of a shared interface
// across backends at all.
#[allow(clippy::unused_async_trait_impl)]
impl ClusterBackend for ProcessBackend {
    type Handle = ProcessHandle;

    async fn spawn(&self, config: &NodeConfig) -> Result<Self::Handle, BackendError> {
        let child = Command::new(&self.daemon_binary)
            .arg("--data-dir")
            .arg(&config.data_dir)
            .arg("--test-run-id")
            .arg(config.test_run_id.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit()) // daemon panics/errors surface in test output
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| BackendError::Spawn(e.to_string()))?;

        Ok(ProcessHandle { child, data_dir: config.data_dir.clone() })
    }

    async fn grpc_endpoint(&self, handle: &Self::Handle) -> Result<String, BackendError> {
        let handshake_path = handle.data_dir.join("daemon.json");
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            if let Ok(bytes) = tokio::fs::read(&handshake_path).await {
                let info: HandshakeInfo = serde_json::from_slice(&bytes)
                    .map_err(|e| BackendError::HandshakeParse(e.to_string()))?;
                return Ok(format!("http://127.0.0.1:{}", info.grpc_port));
            }

            if Instant::now() >= deadline {
                return Err(BackendError::HandshakeTimeout);
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn stop(&self, mut handle: Self::Handle) -> Result<(), BackendError> {
        let _ = handle.child.kill().await;
        Ok(())
    }
}
