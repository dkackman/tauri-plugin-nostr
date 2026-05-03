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

### Phase 2 — Tauri IPC + TypeScript Bindings + Example App

**Goal:** All 8 commands accessible from the frontend; example app fully functional as a sync test harness.

Plugin files created/modified:
- `build.rs` — register all commands
- `src/commands.rs` — implement all command handlers
- `src/desktop.rs` — expose methods used by commands
- `src/lib.rs` — Builder pattern, invoke_handler update
- `permissions/` — add permission entries for all new commands
- `guest-js/index.ts` — full TypeScript API

Commands: `publish`, `fetch`, `sync_all`, `add_relay`, `remove_relay`, `get_relays`, `get_pubkey`, `get_status`

Example app files created/modified:
- `examples/tauri-app/package.json` — add `bootstrap` dependency
- `examples/tauri-app/src/main.js` — import Bootstrap CSS/JS
- `examples/tauri-app/src-tauri/src/lib.rs` — add `set_sync_key` and `generate_sync_key` commands
- `examples/tauri-app/src/App.svelte` — two-column layout shell, Tauri event listeners
- `examples/tauri-app/src/lib/IdentityCard.svelte` — nsec paste + generate key UI
- `examples/tauri-app/src/lib/RelayCard.svelte` — relay list, add/remove
- `examples/tauri-app/src/lib/ActionsCard.svelte` — plugin command buttons
- `examples/tauri-app/src/lib/SettingCard.svelte` — syncable display name + accent color
- `examples/tauri-app/src/lib/ConsoleLog.svelte` — scrollable timestamped log

### Example App Layout

```
┌─────────────────────┬──────────────────────────┐
│  Identity           │  Synced Setting           │
│  ─────────────      │  ──────────────           │
│  Relays             │  Display name: [______]   │
│  ─────────────      │  Accent color: [■]        │
│  Actions            │  [Publish]  [Fetch]        │
│  [Publish Setting]  │                            │
│  [Fetch Setting]    │  Console Log               │
│  [Sync All]         │  ──────────────           │
│  [Get Status]       │  [Clear]                  │
│  [Get Relays]       │  12:01:03 ✓ Published...  │
│  [Get Pubkey]       │  12:01:05 ✓ Fetched...    │
│  [Clear Signer]     │  12:01:07 ✗ Error: ...    │
└─────────────────────┴──────────────────────────┘
```

### Key Injection

`set_signer` is a Rust-side plugin method, not a TypeScript command. The example app adds its own Tauri commands in `src-tauri/src/lib.rs`:

```rust
#[tauri::command]
async fn set_sync_key(app: AppHandle, nsec: String) -> Result<(), String>

#[tauri::command]
async fn generate_sync_key(app: AppHandle) -> Result<KeyInfo, String>
// KeyInfo { nsec: String, pubkey: String }
```

This keeps the plugin API clean (no raw key exposure over IPC) while allowing the test UI to accept user-provided or freshly generated keys.

### Console Log Design

```typescript
type LogLevel = 'info' | 'success' | 'error' | 'event'

interface LogEntry {
  time: string      // HH:MM:SS
  level: LogLevel
  message: string
}
```

| Level | Bootstrap color | Trigger |
|---|---|---|
| `info` | `text-secondary` | Button pressed, command invoked |
| `success` | `text-success` | Command returned result |
| `error` | `text-danger` | Command threw, or `nostr-sync://error` event |
| `event` | `text-info` | Incoming Tauri event |

Auto-scrolls to bottom. Max 200 entries (oldest dropped). Monospace font, dark background.

### Tauri Event Listeners (registered in `App.svelte`)

| Event | Action |
|---|---|
| `nostr-sync://updated` | Log as `event`; if `category === 'display-setting'`, update SettingCard values |
| `nostr-sync://relay-status` | Log as `event` |
| `nostr-sync://error` | Log as `error` |

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

Categories become "known" when `publish` or `fetch` is called for them — they are recorded in `known_timestamps`. `syncAll` iterates over that set; it takes no arguments.

1. Check signer present → `Error::SignerNotSet` if absent
2. For each category key in `known_timestamps`: call `fetch`
3. For each result newer than the local cache: update `known_timestamps`, emit `nostr-sync://updated` (Phase 3); in Phase 2 results are returned as a list
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
