//! The handshake-file format written by a freshly started daemon and
//! read by local test tooling to discover which port the OS actually
//! assigned it.
//!
//! Before this crate existed, `sync-mesh-daemon` and `sync-mesh-testkit`
//! each defined their own copy of this shape independently — identical
//! by convention, not by anything the compiler enforced. Adding a
//! field to one and forgetting the other would have compiled fine and
//! failed silently at runtime. One definition, both sides depend on
//! it, neither one owns it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeInfo {
    pub grpc_port: u16,
    pub pid: u32,
}
