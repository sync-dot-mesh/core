//! The Sync.Mesh sync engine core.
//!
//! This crate is the hexagon's centre: domain types and the *ports*
//! (traits) it needs from the outside world. It has zero I/O of its
//! own — no filesystem access, no networking, no gRPC. Anything that
//! touches the OS or the network is a port here and an adapter in
//! `sync-mesh-daemon`. This is what makes the engine testable without
//! infrastructure and what lets an adapter (a transport, a clock, a
//! change-detection backend) be swapped without touching engine code.

pub mod domain;
pub mod ports;
