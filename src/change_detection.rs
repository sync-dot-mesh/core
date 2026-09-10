//! Change detection scaffold — will wrap the `notify` crate to provide
//! a single debounced event stream across Linux/macOS/Windows instead
//! of three platform-specific watcher implementations.

// Scaffold only — nothing constructs this yet, hence the allow.
// Remove once the real notify-crate wiring lands.
#[allow(dead_code)]
pub struct ChangeEvent {
    pub path: std::path::PathBuf,
}
