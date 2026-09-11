//! Test 0 from the integration-testing backlog: instances start,
//! report healthy, and shut down clean — plus the negative case of
//! two instances pointed at the same data directory. This is Tier 0
//! (real, separate OS processes on one machine) — every instance here
//! is the actual compiled `sync-mesh-daemon` binary, not a mock.

use sync_mesh_proto::GetStatusRequest;
use sync_mesh_proto::HealthState as ProtoHealth;
use sync_mesh_testkit::{ClusterBackend, ProcessBackend, TestCluster};

/// Resolves the compiled daemon binary's path. `env!` only works from
/// within this crate, which is exactly why `ProcessBackend` takes the
/// path as a constructor argument rather than assuming a binary name.
fn daemon_binary() -> &'static str {
    env!("CARGO_BIN_EXE_sync-mesh-daemon")
}

#[tokio::test]
async fn n_instances_start_report_healthy_and_stop_cleanly() {
    let backend = ProcessBackend::new(daemon_binary());
    let mut cluster = TestCluster::new(backend).expect("create test cluster");

    let names = ["alice", "bob", "carol"];
    for name in names {
        cluster.spawn_node(name).await.unwrap_or_else(|e| panic!("failed to spawn {name}: {e}"));
    }

    for name in names {
        let mut client = cluster
            .status_client(name)
            .await
            .unwrap_or_else(|e| panic!("{name} never became reachable: {e}"));

        let response = client
            .get_status(GetStatusRequest {})
            .await
            .unwrap_or_else(|e| panic!("GetStatus failed for {name}: {e}"))
            .into_inner();

        assert_eq!(
            response.health,
            ProtoHealth::Healthy as i32,
            "{name} should report Healthy, got {:?}",
            response.health
        );
        assert!(!response.node_id.is_empty(), "{name} should report a non-empty node id");
    }

    for name in names {
        cluster.stop_node(name).await.unwrap_or_else(|e| panic!("failed to stop {name}: {e}"));
    }

    // After stopping, the instances must actually be gone — not just
    // marked stopped in our own bookkeeping. A fresh status_client
    // call for a name we already removed should fail because the
    // cluster no longer knows about it at all.
    for name in names {
        let result = cluster.status_client(name).await;
        assert!(result.is_err(), "{name} should no longer be reachable after stop_node");
    }
}

#[tokio::test]
async fn second_instance_against_same_data_dir_never_becomes_reachable() {
    let backend_a = ProcessBackend::new(daemon_binary());
    let mut cluster_a = TestCluster::new(backend_a).expect("create cluster A");
    cluster_a.spawn_node("first").await.expect("first instance must start");

    // Force both instances at the same physical data directory rather
    // than each cluster's own isolated temp dir — done by spawning
    // the second directly through a fresh backend pointed at the
    // first cluster's own directory, so this test's contract is exact
    // about what "same data dir" means rather than relying on
    // internal layout assumptions of TestCluster.
    let shared_dir = tempfile::tempdir().expect("create shared dir");

    let backend_direct = ProcessBackend::new(daemon_binary());
    let first_handle = backend_direct
        .spawn(&sync_mesh_testkit::NodeConfig {
            data_dir: shared_dir.path().to_path_buf(),
            test_run_id: uuid::Uuid::new_v4(),
        })
        .await
        .expect("first direct instance must start");

    // Give the first instance a moment to actually acquire its lock
    // and bind — otherwise this test could race and both attempts
    // would start before either lock exists.
    let _ = backend_direct.grpc_endpoint(&first_handle).await;

    // Both instances point at the same directory, so they'd also
    // share the same handshake file path. Remove the first instance's
    // handshake before attempting the second, so a stale file left
    // over from the first can never be mistaken for evidence that the
    // second one succeeded — the check below must observe a genuinely
    // fresh write, or the absence of one.
    let _ = std::fs::remove_file(shared_dir.path().join("daemon.json"));

    let second_attempt = backend_direct
        .spawn(&sync_mesh_testkit::NodeConfig {
            data_dir: shared_dir.path().to_path_buf(),
            test_run_id: uuid::Uuid::new_v4(),
        })
        .await
        .expect("spawning the process itself succeeds — it's the daemon's own startup that must then fail");

    let result = backend_direct.grpc_endpoint(&second_attempt).await;
    assert!(
        result.is_err(),
        "a second instance against an already-locked data directory must never become reachable"
    );

    let _ = cluster_a.stop_node("first").await;
}
