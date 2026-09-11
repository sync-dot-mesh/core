# Sync.Mesh — Architecture

> Masterless, peer-to-peer file synchronisation. No cloud, no account,
> no server. This document is the complete technical reference — what
> each piece does, why it's built that way, and how the pieces fit
> together. The org's [profile README](https://github.com/sync-dot-mesh/.github)
> is the concise, current-shape overview; this is the depth behind it.

**License:** AGPLv3 with an additional attribution term — see
[Licensing](#licensing) and the repo's `LICENSE` file.

---

## Table of Contents

1. [Core Concepts and Mental Model](#1-core-concepts-and-mental-model)
2. [Change Detection — Proxied Access With Fallback](#2-change-detection--proxied-access-with-fallback)
3. [The Sync Algorithm](#3-the-sync-algorithm)
4. [Conflict Resolution](#4-conflict-resolution)
5. [Mobile Compute Offload](#5-mobile-compute-offload)
6. [Communication Protocol — gRPC Wire Contract](#6-communication-protocol--grpc-wire-contract)
7. [Security Model](#7-security-model)
8. [Architecture — Hybrid Rust Core + .NET Shell](#8-architecture--hybrid-rust-core--net-shell)
9. [Project Structure](#9-project-structure)
10. [Persistent Data — Schema and Rationale](#10-persistent-data--schema-and-rationale)
11. [Power and Battery Strategy](#11-power-and-battery-strategy)
12. [Packaging and Distribution](#12-packaging-and-distribution)
13. [Build System and CI](#13-build-system-and-ci)
14. [Testing](#14-testing)
15. [Development Environment](#15-development-environment)
16. [Licensing](#16-licensing)
17. [Implementation Roadmap](#17-implementation-roadmap)

---

## 1. Core Concepts and Mental Model

### What "In Sync" Means — Formally

Two nodes `A` and `B` are **in sync** for a folder when: for every
relative path in the union of both nodes' snapshot records, both
nodes agree on the same content hash for that path.

A **FileSnapshot** is the ground-truth unit of the sync algorithm —
one record per `(folder, path, owner_node)`, storing the Blake3 hash,
size, modification time, and a deleted flag. Every sync decision is
based on comparing these records, never on directly re-inspecting the
filesystem at decision time.

### The Two Sync Paths

Every sync action is one of two flows:

- **Reactive** — a single file changed on this node; propagate that
  one change to peers immediately.
- **Reconciliation** — a peer just came online (or a periodic full
  check runs); compare entire folder states and catch up on anything
  missed while disconnected.

### Value Objects, Carried Into Rust

The original design's discipline around domain primitives holds
exactly as well in Rust as it did in the C# concept it was designed
around — arguably better, since Rust's newtype pattern plus the
type system make "a `NodeId` can never be confused with a `FolderId`"
a compile-time guarantee with zero runtime cost:

```rust
// Small, Copy, value-equality types — the Rust equivalent of a
// `readonly record struct` wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(uuid::Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FolderId(uuid::Uuid);

// ContentHash::EMPTY (64 zero bytes) is the sentinel for a deleted
// file, not an Option<ContentHash> — same reasoning as the original:
// eliminates null/None-check noise on the hot comparison path and
// makes "this file is deleted" an explicit value, not an absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentHash([u8; 32]); // Blake3 digest

impl ContentHash {
    pub const EMPTY: Self = Self([0u8; 32]);
}

// Validated at construction, not after — a RelativePath that
// exists is guaranteed traversal-safe. Rejects `..` segments,
// normalises separators. Illegal states unrepresentable.
pub struct RelativePath(String);

impl RelativePath {
    pub fn parse(input: &str) -> Result<Self, PathError> {
        // normalise, reject traversal, reject absolute paths
        // ...
    }
}
```

**Errors as values throughout.** `NodeUnreachable`, `ConflictDetected`,
`TokenExpired`, `FeatureNotLicensed` are all normal, expected outcomes
in a distributed system — not exceptional conditions. Rust's `Result<T, E>`
is the native expression of this; no separate `Result<T, TError>` type
needs building the way it did in C#, `std::result::Result` already is
that type.

---

## 2. Change Detection — Proxied Access With Fallback

This is the one area where the original vision and what's currently
scaffolded in `sync-mesh-core` genuinely diverged, and it's worth
being explicit about the reconciliation rather than quietly picking a
side.

### The Two Approaches

**Proxied file access** (FUSE on Linux/macOS, a passthrough driver on
Windows) intercepts every write as it happens. The content hash is
computed *while bytes stream through*, finishing the instant the
writing application closes the file — there is no re-read. Change
detection costs zero additional I/O compared to not running Sync.Mesh
at all, and structurally cannot miss an event the way an OS notification
buffer can overflow and drop one under heavy load.

**Native event watching** (inotify/FSEvents/ReadDirectoryChangesW,
wrapped by the `notify` crate) is simpler and universal — no driver to
install, works identically everywhere — but must re-read and re-hash a
file after the OS reports it changed, and depends on the OS's
notification queue not overflowing under sustained heavy write load.

### The Decision: Proxied Where Available, Native Watching as Automatic Fallback

Not either/or. Per platform:

| Platform | Primary | Fallback / Only Option |
|---|---|---|
| Linux | FUSE passthrough (`fuser` crate) | `notify` crate, if FUSE unavailable |
| macOS | FUSE passthrough (macFUSE, via `fuser`) | `notify` crate — **automatic**, not a "please install a driver" prompt |
| Windows | Passthrough via a Rust Dokan/WinFsp binding | `notify` crate, if the driver isn't installed |
| Android | `notify` crate (backed by Android's `FileObserver`) | — this *is* the primary path; there is no viable third-party FUSE mount on Android without root, so proxied access was never on the table here regardless of platform generally |
| iOS | Not currently a committed platform — see note below | |

The important change from the original design: macOS's proxied path
was previously a hard prerequisite with a first-run "please install
macFUSE" prompt. That's now a genuine automatic fallback — the daemon
attempts the proxied mount, and if the driver isn't present or the
mount fails, it transparently drops to `notify`-based watching with no
user action required and no degraded functionality beyond the
change-detection mechanism itself (correctness is identical either
way; only the I/O-efficiency characteristic differs).

**Note on iOS**: the original design included it as a first-class
target (`SyncPlatform.iOS`, a dedicated `NSFilePresenter`-based
interceptor). The org's current platform list (Linux, Windows, macOS,
Android) does not include it. Carried forward here as a documented
possibility, not a current commitment — worth an explicit decision
later rather than silently dropping or silently re-adding it.

### Implementation Notes That Survive the Rewrite

**The `release()`-fires-on-reads-too trap.** A FUSE `release()` call
happens for file descriptors opened for *reading* as well as writing.
Guard with a per-fd flag: set it on the first `write()` call, and only
finalise the hash and emit a change event in `release()` when that
flag is set. Without this, a process merely *opening* a file to read
it would trigger a spurious sync event.

**Concurrent writers to the same fd.** Multiple simultaneous writers
to one file (unusual, but valid) get independent hasher state keyed by
file descriptor. The last writer's `release()` produces the final
hash — correct, because the file's on-disk state after all writes
complete reflects some real interleaving of them, and the last
`release()` hashes whatever is actually on disk at that point.

**Errors, not panics, in the intercept path.** Every FUSE callback must
return a valid errno on failure. An unhandled panic in a FUSE handler
takes down the mount. Every handler body needs explicit error handling
returning an appropriate errno, with the error logged separately.

---

## 3. The Sync Algorithm

### Reactive Path — One File at a Time

```
Change detected (proxied hash-on-write, or notify + re-hash)
  → change event placed on an internal channel
  → sync engine dequeues it

  1. Upsert a new snapshot record for this file (local node)
  2. Evaluate SyncIntensity — if Paused or MetadataOnly: stop
  3. For each active paired peer in this folder:
       a. Notify the peer of the change (metadata only, no content)
       b. Load the peer's last-known snapshot from local state
       c. Decide: identical hashes → no-op
                  local newer     → push to peer
                  remote newer    → pull from peer
                  both changed,
                  different hash  → apply the folder's conflict policy
       d. Execute the decision
  4. Append an audit-log record
```

### Reconciliation Path — Full Folder on Connect

```
A peer comes online
  1. Stream the peer's full snapshot list (server-streaming RPC)
  2. Upsert all received remote snapshots into local state
  3. Diff our snapshots against theirs:
       → files they're behind on
       → files we're behind on
       → conflicts (both changed, different hash)
       → deletions to propagate
  4. Execute all transfers, respecting current SyncIntensity
  5. Emit a sync-completed signal (drives tray/UI status)
```

### The Diff Function — Pure, Deterministic, No I/O

The most important piece of logic in the whole system, and it stays
exactly as small and testable in Rust as the original's pure static
C# function was:

```rust
pub fn compute_diff(
    ours: &[FileSnapshot],
    theirs: &[FileSnapshot],
) -> SnapshotDiff {
    // Three passes: paths in ours (categorise push/pull/conflict),
    // paths only in theirs (pull), paths theirs marks deleted with
    // a newer deletion timestamp (delete locally).
    // No I/O, no side effects, fully deterministic — every case is
    // a pure function of its two inputs.
}
```

The critical edge case, unchanged from the original design: both
nodes modified a file at exactly the same timestamp with different
content. Clocks can genuinely agree to the second while producing
different bytes. This is a real conflict, not a bug in the diff logic —
it goes to the conflict resolver.

### Delta Transfer — rsync Algorithm for Large Files

For files above the delta threshold (10 MB default), a full re-transfer
is replaced by block-level delta transfer:

1. **Receiver** computes a manifest of its existing copy — one entry
   per 4 KB block, containing a cheap rolling checksum (Adler32) plus a
   strong hash (Blake3) for collision resistance.
2. **Sender** streams through the new file computing rolling hashes
   over a sliding 4 KB window. A window matching the manifest (rolling
   hash agrees, strong hash confirms) becomes a reference to the
   existing block; anything else becomes literal bytes.
3. **Receiver** reconstructs the file from the stream: literal blocks
   from the wire, reference blocks from its own existing copy.

Most impactful for SQLite-backed files (password managers, Lightroom
catalogs, anything using SQLite as its file format), large text/Markdown
files, append-heavy logs, and any file that grows rather than being
rewritten wholesale.

---

## 4. Conflict Resolution

### What Causes One

The canonical case: edit a file on a laptop while offline, a
collaborator edits the same file elsewhere, both reconnect. The diff
surfaces this — both modified, different hashes, no clear winner from
timestamps alone.

### Three Policies, Configured Per Folder

**LastWriteWins** (default) — later modification time wins, the loser
is discarded, every decision is written to the audit log so nothing
vanishes without a record. Risk: if system clocks drift by more than a
few seconds, the wrong version can win — NTP mitigates this and is on
by default on every modern OS. Best for folders where typically only
one device edits (camera roll, downloads).

**KeepBoth** — the winner keeps its filename, the loser is renamed
alongside it (`filename.conflict-{node}-{timestamp}.ext`). Nothing is
ever deleted without the user's knowledge. Best for document folders.

**ManualResolution** — neither version is written until the user
decides. A conflict record is created, the tray/UI reflects it, sync
for that specific path is suspended while every other path continues
normally. Best for financial spreadsheets, legal documents, config
files.

The resolver is a pure function of `(local, remote, policy)` — no
hidden state, fully testable without any I/O.

---

## 5. Mobile Compute Offload

This is the mechanism behind what's referenced elsewhere as "offload
computation" — worth being precise, since it's a specific two-tier
role system, not general N-peer work distribution.

### Roles

Every node is assigned a role at the start of each sync cycle, based
on current device state — not static, re-evaluated as conditions
change:

- **Lightweight** — default for mobile. Does not run the diff
  algorithm locally.
- **Capable** — default for desktop platforms; also any mobile device
  that's plugged in, on WiFi, and has this enabled. Runs the full diff
  algorithm and serves offload requests from lightweight peers.

### The Offload Flow

```
Mobile node, Lightweight role:
  1. Collect a raw manifest for the folder — path, size, modified
     time for each file. No hashing, no content read.
  2. Find a reachable Capable peer (via status/heartbeat data).
  3. Send the manifest via a single RPC.

Capable peer, on receiving the manifest:
  1. Build a synthetic snapshot list from the incoming manifest.
  2. Load its own snapshot records for the folder.
  3. Run the diff function — exactly the same pure function used
     for the reconciliation path — against (own snapshots, incoming
     manifest).
  4. Apply conflict policy.
  5. Return the instruction set: push-to-mobile[], pull-from-mobile[],
     delete-locally[], conflicts[].

Mobile node, executing instructions:
  - No diff computation ever happens on the mobile device.
  - Pure network transfer + local state updates from here.
```

If the capable peer disconnects mid-offload, the mobile node falls
back to `MetadataOnly` for that cycle and retries the offload on the
next one — no data loss, just deferred computation.

### Role Reassignment

A mobile device that's plugged in, on WiFi, and has opted in can
itself become eligible as a Capable peer for *other* lightweight
devices — the role is a function of current state, not a fixed
property of the platform.

---

## 6. Communication Protocol — gRPC Wire Contract

All inter-node communication is gRPC over HTTP/2 with TLS 1.3.
`protos/sync.proto` is the single source of truth for the wire format.
On the Rust side, bindings are generated at build time via `tonic-build`
+ `prost` (the direct equivalents of `Grpc.Tools`' code generation).

### The RPCs

**Pairing** — `Pair` (unary): exchange node identity and certificate
fingerprint, validate a time-limited pairing token.

**Status** — `GetStatus` (unary): platform, battery level, free disk,
whether this node can serve as a compute peer, whether it prefers
offload. Used by heartbeats and by role assignment.

**Snapshot exchange** — `GetSnapshot` (server-streaming): the server
streams one entry per file rather than batching — a folder with 50,000
files never requires holding all 50,000 records in memory on either
side simultaneously.

**Change notification** — `NotifyChange` (unary): metadata-only "I
changed this file" signal; the receiver decides whether to act based
on its own snapshot state.

**File transfer** — `PushFile` (client-streaming) for whole-file
transfer with atomic temp-file-then-rename on the receiving side;
`PushDelta` (client-streaming) for the block-level rsync transfer.

**Realtime watch** — `WatchChanges` (server-streaming): a long-lived
stream pushing future change events to a subscribed peer — active
desktop nodes use this for near-zero-latency notification instead of
polling.

**Mobile offload** — `OffloadDiff` (unary): the RPC underlying the
compute-offload flow in §5.

### Debuggable With Standard Tools

Because it's plain HTTP/2 gRPC, not a custom binary protocol, it's
inspectable with off-the-shelf tooling:

```bash
grpcurl -plaintext localhost:50051 list
grpcurl -plaintext localhost:50051 syncmesh.v1.SyncService/GetStatus
```

---

## 7. Security Model

### Node Identity

Each node generates an Ed25519 key pair on first run. The public key
becomes a self-signed certificate; its fingerprint (SHA-256 of the
DER-encoded public key) is the node's long-term cryptographic identity —
exchanged during pairing, pinned, and verified on every TLS connection.
A certificate that doesn't match the pinned fingerprint is rejected.
No PKI, no CA, no expiry to manage. This design is unchanged by the
language shift — Ed25519 and certificate pinning are equally natural
in Rust (via `ring` or `ed25519-dalek`) as they were in .NET.

### Private Key Storage

The original design used ASP.NET Core's Data Protection API for
at-rest encryption of the node's private key. That's a .NET-specific
mechanism; the Rust core needs its own equivalent. Options under
consideration, not yet finalised: OS keychain integration
(`keyring` crate, backed by libsecret/Keychain/Credential Manager per
platform) versus a Rust-native encrypted-at-rest scheme (`age` or
similar) mirroring the original's approach of encrypting the key file
directly. This is a real open decision, called out rather than
silently resolved.

### Pairing Token Security

A time-limited, single-use, cryptographically signed token is
generated for QR-code pairing — 60-second expiry, consumed on first
successful use. Even a photographed QR code has only a narrow window
before it's worthless.

### What Is Not Encrypted

Files at rest are not encrypted by Sync.Mesh — it syncs files as-is.
At-rest encryption is the OS's job (LUKS/BitLocker/FileVault); Sync.Mesh
syncs whatever's inside such a volume transparently. Traffic between
nodes is TLS 1.3 in transit regardless.

### Licence Key Security

Git Sync licences are signed tokens (EdDSA), validated entirely
offline against a public key bundled in the binary — no outbound
network call, no phone-home. See §16 for how this interacts with the
project's actual license choice.

---

## 8. Architecture — Hybrid Rust Core + .NET Shell

### Why Hybrid

Full reasoning lives in the backlog's Rust-vs-.NET evaluation entry.
The short version: Rust for the sync engine specifically — change
detection, hashing, delta computation, transport — where correctness
guarantees around concurrency and raw throughput matter most. .NET
(MAUI/Blazor Hybrid) for everything platform-and-UI-facing — the tray,
the settings panel, dependency injection, persistence, configuration —
where ecosystem depth and Android's actual native story matter most.
Communication between the two happens via FFI.

### What This Replaces

The original design's Onion Architecture (pure C# domain with zero
external dependencies) and Vertical Slice Architecture (one
self-contained feature folder per user action) were good ideas, and
the underlying *reasoning* — dependency inversion, no cross-cutting
coupling, test without infrastructure — carries forward. What changes
is where the boundary sits:

```
.NET MAUI/Blazor Hybrid UI, ViewModels        ← was: Sync.Mesh.Presentation
  depends on ↓
.NET Features (vertical slices, handlers)      ← was: Sync.Mesh.Features
  depends on ↓
.NET Daemon-side services: DI, EF Core,
config, platform tray/device-info integration  ← was: Sync.Mesh.Daemon
  depends on ↓  (FFI boundary)
Rust engine crate: change detection, hashing,
delta computation, transport, sync algorithm   ← was: Sync.Mesh.Core + parts of Daemon
```

The Rust engine crate is the new "innermost ring" in spirit — it owns
the domain logic that must be provably correct and fast, has minimal
dependencies, and knows nothing about EF Core, Blazor, or platform
tray APIs. The .NET side owns everything that benefits from that
ecosystem's depth and calls into the Rust engine across FFI for the
actual sync work.

**Persistence stays on the .NET side.** Per the hybrid-architecture
decision, SQLite/EF Core remains .NET's responsibility, not the Rust
engine's — the Rust core is computation and transport, largely
stateless itself, receiving what it needs through the FFI call and
returning results the same way. Exact FFI boundary shape (flat C-ABI
structs vs. serialised payloads for the richer types like `SnapshotDiff`)
is implementation detail to work out per call, not a fixed contract
yet.

---

## 9. Project Structure

Two toolchains now, not one. The Rust side:

```
sync-mesh-core/                    (Cargo workspace)
├── crates/
│   ├── engine/                    ← lib: domain, diff algorithm, conflict
│   │                                 resolution, delta transfer — minimal deps
│   ├── daemon/                    ← bin: thin wrapper, gRPC server, FFI exports
│   └── testkit/                   ← dev-dependency only: TestCluster + backends
├── protos/
│   └── sync.proto                 ← single source of truth for the wire format
```

The .NET side (shell, UI, platform integration) is a separate solution
consuming the Rust engine as a native library dependency — its own
project tree, not detailed here since it hasn't been scaffolded yet.

---

## 10. Persistent Data — Schema and Rationale

One SQLite file, zero external process, owned by the .NET side.

| Platform | Default location |
|---|---|
| Linux/macOS | `~/.syncmesh/syncmesh.db` |
| Windows | `%APPDATA%\SyncMesh\syncmesh.db` |
| Android | App-private storage |
| Override | `SYNCMESH_DATA_DIR` environment variable |

**`node_identities`** — one row for the local device, one per trusted
peer. Key columns: pinned certificate fingerprint, offload-preference
and compute-peer-eligibility flags (drive role assignment), last-seen
timestamp, active flag (set false when heartbeats fail, engine skips
inactive nodes).

**`sync_folders` + `sync_folder_nodes`** — watched directories, and a
junction table for which nodes are authorised for which folder. A node
can be paired for some folders but not others.

**`file_snapshots`** — the most important table. One row per
`(folder_id, path, owner_node)` — including *remote* peers' snapshots,
not just local files. When a peer's snapshot is received, it's written
here, which is what makes the diff function able to run without a live
connection: the last-known state of every peer's every file is already
on hand. `ContentHash::EMPTY`, not NULL, represents deleted files, so
deletions participate in hash comparisons without null-handling.

**`sync_conflicts`** — populated on ManualResolution or KeepBoth.
Never deleted after resolution, only updated with the resolution and a
timestamp — history is preserved.

**`sync_events`** — append-only audit log. Powers the History UI. No
automatic pruning in the free tier; Git Sync's retention window also
prunes corresponding event records as part of its own maintenance.

---

## 11. Power and Battery Strategy

Sync intensity is evaluated fresh at the start of every sync operation —
a pure function of device state and settings, first-match-wins priority:

1. Manually paused by the user → **Paused**
2. Battery critical, not charging → **MetadataOnly**
3. Battery low, not charging, and the setting is enabled → **Reduced**
   (default threshold 20%)
4. No network connectivity → **Paused**
5. On mobile data and not explicitly allowed → **MetadataOnly**
   (off by default)
6. Otherwise → **Full**

**Full** — everything, delta sync active above the size threshold, Git
Sync commits run each cycle. **Reduced** — large files (>50 MB)
deferred, Git Sync commits suspended, delta sync still active for
eligible files. **MetadataOnly** — snapshot exchange only, zero
content bytes transferred; changes accumulate and catch up
automatically once intensity returns to Full. **Paused** — everything
stops except the heartbeat (status-only, no transfers), so peer
availability still shows correctly in the UI.

### Android-Specific Constraints

**Doze mode** — the OS suspends background work when the screen is
off and the device is stationary. Detected via the platform's battery
API; the engine responds by dropping to MetadataOnly, and re-evaluates
once Doze ends.

**Foreground service requirement** — without an active foreground
service, Android can kill the process at any time with no warning.
The persistent, non-dismissible notification (required by the
platform for foreground services) is what keeps the daemon alive, and
doubles as the tray equivalent on a platform with no actual tray.

---

## 12. Packaging and Distribution

Every platform's package now needs to bundle the Rust engine's
compiled native library alongside the .NET publish output, not just
a self-contained .NET binary.

**Linux** — `.deb` (binary + systemd unit + dependency on FUSE
userspace libraries where the proxied path is available),
`.rpm` (equivalent via `rpmbuild`), `.AppImage` (no system integration,
runs anywhere), NixOS flake (reproducible build via `buildDotnetApplication`
plus a Rust build step for the engine).

**Windows** — InnoSetup installer bundling both the .NET publish
output and the Rust engine DLL, plus the passthrough driver installer
if the proxied path is used; MSIX for enterprise/Store deployment.

**macOS** — signed and notarized `.pkg`/`.dmg`. No hard macFUSE
prerequisite anymore per §2 — the proxied path degrades automatically
if the driver isn't present, so packaging doesn't need to gate on it.

**Android** — APK/AAB via MAUI, with the Rust engine compiled for the
relevant Android ABI(s) and bundled as a native library inside the
package.

### Adding a New Platform

Unchanged in spirit from the original design: implement the platform's
device-info and tray/notification equivalents on the .NET side,
implement or select the appropriate change-detection backend on the
Rust side (proxied where viable, `notify`-based otherwise), wire it up.
The sync algorithm, the protocol, and the UI layer need no changes.

---

## 13. Build System and CI

Two toolchains means CI needs both. On every push: `cargo test` +
`cargo clippy` + `cargo fmt --check` for the Rust engine (already
live in `sync-dot-mesh/core`'s CI), plus the equivalent `dotnet build`
+ `dotnet test` for the .NET shell once that solution exists. On tag
push: matrix publish per platform, each job building the Rust engine
for that target first, then the .NET publish consuming it, then
packaging.

---

## 14. Testing

Superseded by, and should be read alongside, the dedicated
[Integration Testing Strategy](https://github.com/sync-dot-mesh/.github/tree/main/brainstorms)
backlog entry — the tiered (process → container → remote → Android),
pluggable-backend `testkit` design there is the actual current plan,
replacing this document's original single-tier, C#-only
`Sync.Mesh.Integration.Tests`/`TestNode` concept. The core ideas
survive (real spawned instances, real temp directories, no mocking the
thing you're trying to prove works) — the mechanism is more capable
than what was originally sketched.

---

## 15. Development Environment

The Rust engine's dev environment is the Nix + direnv + isolated
VSCodium setup already live in `sync-dot-mesh/core` (see that repo's
own README) — `./init_devenv.sh` once, then `cd` into the directory
loads everything automatically from then on. The .NET shell will need
its own equivalent once that solution exists, likely extending the
same `shell.nix` to add the .NET SDK alongside the Rust toolchain
already there, rather than a separate, disconnected environment.

---

## 16. Licensing

**AGPLv3**, with an additional term under GPLv3/AGPLv3 Section 7(b)
requiring preservation of author attribution. Full text in the repo's
`LICENSE` file. This replaces the original "Core MIT, paid features
source-available" plan.

Why the change: AGPL guarantees the entire codebase — including any
paid-feature code — stays open source through any fork, which a
permissive license like MIT does not enforce, and which a
source-available license (BSL, SSPL, etc.) explicitly trades away in
the other direction. Paid features (Git Sync) work under this model as
an officially-issued, signed license key plus ongoing support — the
gating code is itself open and technically forkable, which is an
accepted consequence of the license, not a loophole. AGPL's specific
addition over plain GPL — closing the "modify and run as a network
service without sharing changes" gap — matters here given the
project's optional relay tier and license-issuance service.

---

## 17. Implementation Roadmap

Milestones re-sequenced around the Rust-first engine work, since the
sync algorithm's correctness is the thing that must be solid before
any UI or platform integration is worth building on top of it.

**Milestone 1 — Engine domain.** Value objects, the diff function, the
conflict resolver, sync intensity evaluation — all as pure functions
with exhaustive unit tests, no I/O, running in milliseconds. This
is the same discipline the original design insisted on for Core, now
in Rust: if the diff function has an edge-case bug, it produces
subtle, silent data loss in production. Get this provably right first.

**Milestone 2 — Change detection.** The proxied/fallback interceptor
design from §2, starting with Linux (the most mature `fuser`/notify
tooling), with an integration test verifying the emitted hash matches
an independently-computed reference hash and that no re-read occurs
on the proxied path specifically.

**Milestone 3 — Transport.** The complete `sync.proto`, `tonic`-generated
bindings, the gRPC server and client, mDNS discovery. Testable
in-process with two loopback channels before any real multi-instance
work is needed — this is exactly Test 0/1 from the testing-strategy
backlog.

**Milestone 4 — Sync engine integration.** Wire change detection to
the sync algorithm to the transport layer. This is where the
integration-testing tiers actually start exercising real multi-instance
behavior — two real processes, real temp directories, a file created
on one appearing on the other.

**Milestone 5 — FFI boundary + .NET shell scaffold.** Define the
actual FFI contract, stand up the minimal .NET shell that calls into
the Rust engine, gets a UI-less end-to-end path working before any
Blazor/tray work begins.

**Milestone 6 — Persistence, security, pairing.** SQLite schema on
the .NET side, node identity generation, certificate pinning, the
pairing flow.

**Milestone 7 — Mobile offload.** The Lightweight/Capable role system
from §5, tested with a simulated lightweight node offloading to a
simulated capable one.

**Milestone 8 — Tray, UI, packaging.** Platform tray/notification
implementations, the Blazor settings UI, packaging per platform
including the two-toolchain build.

**Milestone 9 — Git Sync (paid feature).** Shadow repo via a Rust git
library or `git2`/libgit2 bindings, offline license validation, the
history/restore UI.
