use std::path::PathBuf;

/// One detected filesystem change.
///
/// This is a plain domain value type — it says nothing about *how* the
/// change was detected. Per `ARCHITECTURE.md` §2, detection itself will
/// be a port (proxied file access where available, falling back
/// automatically to native OS event watching where it isn't, always on
/// Android) with the adapter choice made per-platform in `daemon`. That
/// port doesn't exist yet because there is no second real detection
/// mechanism to abstract over yet — introducing the trait before then
/// would be an abstraction with only one implementation, which is
/// speculative rather than earned. This struct is the shared shape
/// both eventual adapters will need to produce.
#[allow(dead_code)] // nothing constructs this yet — first real user is Milestone 2
pub struct ChangeEvent {
    pub path: PathBuf,
}
