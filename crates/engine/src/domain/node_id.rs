use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

/// A node's long-term cryptographic identity.
///
/// This wraps a [`Uuid`] rather than passing raw UUIDs (or worse, raw
/// `String`s) around, for one specific reason: once folders, peers,
/// and pairing tokens all get their own ID types (`FolderId`,
/// `PairingToken`, ...), every one of them is a UUID underneath, and
/// without distinct wrapper types the compiler cannot stop a
/// `FolderId` from being passed where a `NodeId` was expected — they'd
/// all just be `Uuid`. With a newtype per concept, that mix-up is a
/// compile error, not a runtime bug discovered when node A somehow
/// gets treated as folder B. See the architecture doc's domain
/// primitives section — the same reasoning as `ContentHash` and
/// `RelativePath`, applied to identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(Uuid);

impl NodeId {
    /// Generates a fresh, random node identity.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing UUID as a `NodeId` — for reconstructing an
    /// identity read back from storage, not for minting new ones (use
    /// [`NodeId::new`] for that).
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for NodeId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_display_and_from_str() {
        let id = NodeId::new();
        let text = id.to_string();
        let parsed: NodeId = text.parse().expect("valid NodeId text must parse");
        assert_eq!(id, parsed);
    }

    #[test]
    fn two_fresh_ids_are_never_equal() {
        // Not a mathematical guarantee (UUIDv4 collisions are merely
        // astronomically unlikely, not impossible) — but a collision
        // here would indicate a broken RNG, worth catching.
        assert_ne!(NodeId::new(), NodeId::new());
    }
}
