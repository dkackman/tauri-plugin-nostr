# tauri-plugin-nostr-sync — Implementation Design

## Context

This document describes the phased implementation plan for `tauri-plugin-nostr-sync`, a Tauri 2.x plugin providing encrypted, decentralized state sync across app instances via Nostr replaceable events (NIP-33, kind 30078) with NIP-44 encryption.

The canonical spec is `specs/tauri-plugin-nostr.md`. This design document covers *how* to implement it: phasing decisions, component structure, data flow, and test strategy.

---

## Decisions Made

- **Phased approach** — three phases, each independently testable and mergeable
- **Desktop-first** — mobile.rs remains a stub; mobile implementation is a future phase
- **Rename now** — plugin renamed from `tauri-plugin-nostr` to `tauri-plugin-nostr-sync` in Phase 1 (affects Cargo.toml, build.rs, lib.rs IPC prefix, TypeScript invoke calls)
- **State-first (Option A)** — Phase 1 builds and tests `NostrSyncState` in pure Rust with no Tauri IPC; Phase 2 wires IPC; Phase 3 adds outbox and events

---

## Phase Breakdown

### Phase 1 — Core State Machine (pure Rust, no Tauri IPC)

**Goal:** `NostrSyncState` is correct, tested, and has no Tauri dependencies beyond what the plugin infrastructure requires.

Files created/modified:
- `Cargo.toml` — rename + add dependencies
- `build.rs` — update plugin name
- `src/lib.rs` — rename plugin ID, update ext trait name
- `src/error.rs` — expand error enum
- `src/models.rs` — replace ping types with real IPC models
- `src/state.rs` — NEW: `NostrSyncState` implementation
- `src/desktop.rs` — thin wrapper delegating to `NostrSyncState`

**Phase 1 does NOT include:** Tauri commands, TypeScript bindings, outbox, events, relay subscriptions for receive flow.

---

### Phase 2 — Tauri IPC + TypeScript Bindings

**Goal:** All 8 commands accessible from the frontend; example app can call them.

Files created/modified:
- `build.rs` — register all commands
- `src/commands.rs` — implement all command handlers
- `src/desktop.rs` — expose methods used by commands
- `src/lib.rs` — Builder pattern, invoke_handler update
- `permissions/` — add permission entries for all new commands
- `guest-js/index.ts` — full TypeScript API

Commands: `publish`, `fetch`, `sync_all`, `add_relay`, `remove_relay`, `get_relays`, `get_pubkey`, `get_status`

---

### Phase 3 — Outbox, Events, Integration Tests

**Goal:** Reliable delivery with retry; frontend receives real-time updates.

Files created/modified:
- `src/outbox.rs` — NEW: `OutboxQueue` (persisted JSONL, exponential backoff, 10-attempt limit)
- `src/state.rs` — integrate outbox into publish flow; add receive subscription loop; emit Tauri events
- `tests/` — Level 2 integration tests with mock relay
- `tests/e2e/` — Level 4 E2E tests (gated with `#[ignore = "requires local relay"]`)

Events emitted: `nostr-sync://updated`, `nostr-sync://relay-status`, `nostr-sync://error`

---

## Architecture

### File Structure (after all phases)

```
src/
  lib.rs          # plugin registration, Builder, TauriPluginNostrSyncExt trait
  commands.rs     # Tauri IPC command handlers (thin — delegate to state)
  desktop.rs      # TauriPluginNostrSync<R> struct, wraps NostrSyncState
  mobile.rs       # stub (returns PluginInvokeError)
  state.rs        # NostrSyncState — core logic (Phase 1)
  outbox.rs       # OutboxQueue — retry queue (Phase 3; backoff fn defined in Phase 1)
  models.rs       # Serde types for IPC
  error.rs        # Error enum
guest-js/index.ts # TypeScript bindings (Phase 2)
```

### `NostrSyncState` (core struct, `src/state.rs`)

```rust
pub struct NostrSyncState {
    namespace: String,
    device_id: String,                                              // uuid v4, ephemeral
    client: nostr_sdk::Client,                                      // relay pool + subscriptions
    signer: RwLock<Option<Arc<dyn NostrSigner + Send + Sync>>>,    // swappable at runtime
    known_timestamps: RwLock<HashMap<String, Timestamp>>,           // category → last seen
}
```

`nostr_sdk::Client` owns relay connections — no raw WebSocket management in this plugin.

The signer is behind `RwLock<Option<...>>` so it can be injected after wallet unlock without reconstructing the client.

---

## Data Flow

### Publish

1. Check signer present → `Error::SignerNotSet` if absent
2. Serialize payload to JSON string
3. Check size ≤ 64KB → `Error::PayloadTooLarge` if exceeded
4. NIP-44 encrypt with signer's public key
5. Build NIP-33 event: kind `30078`, d-tag `{namespace}/{category}/v1`, content = ciphertext
6. Sign + broadcast via `client.send_event(event)`
7. Return immediately (Phase 1: fire-and-forget; Phase 3: outbox on failure)

### Fetch

1. Check signer present → `Error::SignerNotSet` if absent
2. Build filter: kind `30078`, author = own pubkey, `#d` = d-tag for category
3. `client.get_events_of(filter, timeout)` → list of events
4. Take event with latest `created_at`
5. NIP-44 decrypt content
6. Deserialize JSON → return `FetchResult { payload, updated_at, device_id }`

### SyncAll (Phase 2)

1. Check signer present → `Error::SignerNotSet` if absent
2. For each known category in `known_timestamps` (plus any the host passes): call `fetch`
3. For each result newer than the local cache: emit `nostr-sync://updated` (Phase 3) or return results (Phase 2)
4. Returns when all fetches complete

### Receive (Phase 3)

1. On relay connect: subscribe to all kind `30078` events for own pubkey
2. On event received: NIP-44 decrypt
3. Compare `created_at` with `known_timestamps[category]`
4. If newer: update cache, emit `nostr-sync://updated` to frontend
5. If older or equal: discard

### Outbox Retry (Phase 3)

- JSONL file in app data directory
- Each entry: `{ category, encrypted_event, attempts, last_attempt_at }`
- Retry loop triggers on: startup, keypair injection, relay reconnect
- Backoff: `min(2^attempts seconds, 300s)`
- Remove entry on delivery to at least one relay
- Abandon after 10 attempts: emit `nostr-sync://error`, remove entry

---

## Error Variants

```rust
pub enum Error {
    Io(std::io::Error),
    Nostr(nostr_sdk::client::Error),
    SignerNotSet,
    PayloadTooLarge { size: usize, limit: usize },
    EncryptionFailed(String),
    DecryptionFailed(String),
    InvalidNamespace(String),
    #[cfg(mobile)]
    PluginInvoke(tauri::plugin::mobile::PluginInvokeError),
}
```

---

## Cargo Dependencies (added in Phase 1)

| Crate | Purpose |
|---|---|
| `nostr-sdk` | Relay client, NIP-44, NIP-01, `NostrSigner` trait |
| `nostr` | Core Nostr types (`Event`, `Keys`, `Timestamp`, etc.) |
| `serde_json` | Payload serialization |
| `tokio` | Async (Tauri's runtime) |
| `chrono` | Timestamps in IPC models |
| `uuid` | Device ID generation |
| `zeroize` | Explicit zeroize calls if needed beyond ZeroizeOnDrop |

---

## Testing Strategy

### Phase 1 — Level 1 Unit Tests (in `src/state.rs` and `src/outbox.rs`)

All pure logic, no I/O, no network.

| Test | Assertion |
|---|---|
| `dtag_format` | `build_dtag("sage", "ui-settings")` → `"sage/ui-settings/v1"` |
| `namespace_rejects_slash` | namespace containing `/` → `InvalidNamespace` |
| `namespace_rejects_empty` | empty namespace → `InvalidNamespace` |
| `payload_encrypt_decrypt_roundtrip` | encrypt → decrypt with same ephemeral key → original value |
| `payload_too_large` | 65KB payload → `PayloadTooLarge` |
| `payload_at_limit` | exactly 64KB → no error |
| `backoff_calculation` | attempts 0–10 → `min(2^n, 300)` seconds |
| `sync_status_not_ready_no_signer` | `SyncStatus.ready == false` when signer absent |
| `timestamp_newer_wins` | newer `created_at` updates cache |
| `timestamp_older_discarded` | older `created_at` leaves cache unchanged |

### Phase 2 — Level 3 Tauri Command Tests

Using `tauri::test` with `MockRuntime`. Validates IPC surface: command names, argument shapes, error serialization.

### Phase 3 — Level 2 Integration + Level 4 E2E

Level 2: in-process mock relay via `tokio-tungstenite`. Tests publish/receive flow, outbox retry, multi-relay behavior.

Level 4: local relay (Docker), two `NostrSyncState` instances. Gated with `#[ignore = "requires local relay"]`. Runs on merge to main.

---

## Constraints Carried Forward from Spec

- Signer is never a raw `SecretKey` — held as `Arc<dyn NostrSigner + Send + Sync>` (Arc needed for shared ownership with `nostr_sdk::Client`); never cloned into plain structs
- `clear_signer()` replaces the `Arc` with `None`; when the last `Arc` ref drops, `ZeroizeOnDrop` on the underlying impl zeroes key bytes
- Outbox stores encrypted ciphertext only — never plaintext
- `syncAll()` is never called automatically — host app calls it explicitly after keypair injection
- d-tag format is fixed: `{namespace}/{category}/v1`
- 64KB limit is global and not user-configurable
