//! Generated gRPC bindings for `sync.proto`. Deliberately its own
//! crate rather than living inside the daemon binary — the wire
//! format is a contract both the server (`sync-mesh-daemon`) and any
//! client (`sync-mesh-testkit`, and later a real peer/UI client)
//! depend on equally; it belongs to neither of them specifically.

// tonic::Status is a genuinely large type, and clippy's
// result_large_err lint fires on every RPC method tonic-build
// generates below — this is tonic's own generated code, not
// something we wrote, and it regenerates on every build, so the
// lint is suppressed at this boundary rather than something we could
// "fix" in the usual way.
#![allow(clippy::result_large_err)]

tonic::include_proto!("syncmesh.v1");
