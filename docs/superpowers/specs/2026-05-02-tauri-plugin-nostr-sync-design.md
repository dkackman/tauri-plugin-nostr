# tauri-plugin-nostr-sync — Implementation Design

## Context

This document describes the phased implementation plan for `tauri-plugin-nostr-sync`, a Tauri 2.x plugin providing encrypted, decentralized state sync across app instances via Nostr replaceable events (NIP-33, kind 30078) with NIP-44 encryption.

The canonical spec is `specs/tauri-plugin-nostr.md`. This design document covers *how* to implement it: phasing decisions, component structure, data flow, and test strategy.

---

## Decisions Made

- **Phased approach** — three phases, each independently testable and mergeable
- **Desktop-first** — mobile.rs remains a stub; mobile implementation is a future phase
- **Rename now** — plugin renamed from `tauri-plugin-nostr` to `tauri-plugin-nostr-sync` in Phase 1 (affects Cargo.toml, build.rs, lib.rs IPC prefix, TypeScript invoke calls)
- **State-first (Option A)** — Phase 1 builds and tests `NostrSyncState` in pure Rust with no Tauri IPC; Phase 2 wires IPC; Phase 3 adds receive subscription + Tauri events
- **No durable outbox** — `publish` returns `Result` synchronously. Transient relay drops are handled by `nostr-sdk`'s built-in auto-reconnect. Durable cross-restart publish is a host-app concern.
- **Lean on `nostr_sdk::Client`** — the Client owns the signer slot (`set_signer`/`signer`/`unset_signer`) and the relay pool. The plugin holds no parallel signer field and no parallel relay map.

---

## Phase Breakdown

### Phase 1 — Core State Machine (pure Rust, no Tauri IPC) — COMPLETE

**Goal:** `NostrSyncState` is correct, tested, and has no Tauri dependencies beyond what the plugin infrastructure requires.

Files created/modified:
- `Cargo.toml` — rename + add dependencies
- `build.rs` — update plugin name
- `src/lib.rs` — rename plugin ID, update ext trait name
- `src/error.rs` — expand error enum
- `src/models.rs` — replace ping types with real IPC models
- `src/state.rs` — NEW: `NostrSyncState` implementation
- `src/desktop.rs` — thin wrapper delegating to `NostrSyncState`

**Phase 1 does NOT include:** Tauri commands, TypeScript bindings, events, relay subscriptions for receive flow.

**Post-implementation simplifications applied:** the parallel `signer: RwLock<Option<Arc<dyn NostrSigner>>>` field, the manual `connect_relay` after `add_relay`, the `known_timestamps` cache check inside `fetch`, and the `outbox.rs` stub were all removed once the design surfaced as duplicating nostr-sdk capabilities. The `is_newer` helper and its unit tests went with the cache check; Phase 3 will reintroduce equivalent dedup logic at the receive subscription layer where it belongs.

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

### Phase 3 — Receive Subscription, Events, Integration Tests

**Goal:** Frontend receives real-time updates from remote devices; integration coverage on a mock relay; E2E coverage on a local relay.

Files created/modified:
- `src/state.rs` — add receive subscription task driven by `Client::notifications()`; add receive-side dedup (see decision below); emit Tauri events via the AppHandle
- `src/desktop.rs` — wire the AppHandle to the state so events can be emitted
- `tests/` — Level 2 integration tests with mock relay (publish/receive, multi-relay, all-relays-down)
- `tests/e2e/` — Level 4 E2E tests (gated with `#[ignore = "requires local relay"]`)

Events emitted: `nostr-sync://updated`, `nostr-sync://relay-status`, `nostr-sync://error`

#### Decision to make before implementing: dedup mechanism

`nostr-sdk` ships a `NostrDatabase` trait. `Client::builder().database(MemoryDatabase::default())` (or the sqlite-backed `NostrLMDB` / `NdbDatabase` from sibling crates) attaches a store that already:

- Dedupes events by id.
- Honours NIP-33 replaceable-event semantics — only the latest `(pubkey, kind, d-tag)` is retained.
- Is queryable via `client.database().query(filter)`.

Two options for Phase 3:

**Option A — Hand-rolled `last_seen: RwLock<HashMap<String, Timestamp>>`.** Simpler to reason about; one HashMap of `category → Timestamp`. In-memory only; resets each launch (which is correct behavior for "emit `updated` on app resume"). Roughly 30 lines of code.

**Option B — Wire `MemoryDatabase` into the client.** Get dedup, replaceable-event semantics, and filter queries for free. Receive handler becomes: on event arrival, check whether the database already has it (it will in the second-delivery case across multiple relays); if not, emit `updated`. Slightly more code in setup, less in the dedup path. Also opens the door to swapping in the sqlite backend later for cross-restart dedup if a use case emerges.

**Recommendation:** Option A is consistent with "transport only — no persistence" and fits the current scope. Pick Option B only if a Phase 4 requirement appears (e.g., "don't re-emit `updated` for events seen in a previous session"). Don't pre-emptively reach for the database — the HashMap is honest about what we actually need today.

---

## Architecture

### File Structure (after all phases)

```
src/
  lib.rs          # plugin registration, Builder, TauriPluginNostrSyncExt trait
  commands.rs     # Tauri IPC command handlers (thin — delegate to state)
  desktop.rs      # TauriPluginNostrSync<R> struct, wraps NostrSyncState
  mobile.rs       # stub (returns PluginInvokeError)
  state.rs        # NostrSyncState — core logic
  models.rs       # Serde types for IPC
  error.rs        # Error enum
guest-js/index.ts # TypeScript bindings (Phase 2)
```

### `NostrSyncState` (core struct, `src/state.rs`)

```rust
// Phase 1 (current)
pub struct NostrSyncState {
    namespace: String,
    device_id: String,             // uuid v4, ephemeral (per-process)
    client: nostr_sdk::Client,     // owns relay pool AND signer slot
}

// Phase 3 adds
//   last_seen: RwLock<HashMap<String, Timestamp>>  // category → last seen, in-memory only
```

`nostr_sdk::Client` owns relay connections AND the signer. The plugin calls `Client::set_signer` / `Client::unset_signer` / `Client::signer` directly; there is no parallel signer field. This means the signer is swappable at runtime without reconstructing the client, and `ZeroizeOnDrop` on the underlying `Keys` fires when the last `Arc` reference inside the client drops.

The client is built with `Options::default().autoconnect(true)`, so `add_relay` automatically opens the connection. `nostr-sdk` then handles reconnection on its own (`reconnect=true`, `retry_interval=10s`, `adjust_retry_interval=true` per `RelayOptions` defaults).

---

## Data Flow

### Publish

1. Get signer via `client.signer().await` → `Error::SignerNotSet` if absent
2. Serialize payload to JSON string; check size ≤ 64KB → `Error::PayloadTooLarge` if exceeded
3. NIP-44 encrypt with signer's own public key
4. Build NIP-33 event: kind `30078`, d-tag `{namespace}/{category}/v1`, `device_id` tag, content = ciphertext
5. Sign via the signer's `sign_event`
6. Broadcast via `client.send_event(event)`
7. If `Output.success` non-empty → `Ok(())`; if zero relays accepted → `Err`. Caller decides whether to retry.

### Fetch

1. Get signer via `client.signer().await` → `Error::SignerNotSet` if absent
2. Build filter: kind `30078`, author = own pubkey, `#d` = d-tag for category
3. `client.fetch_events(vec![filter], timeout)` → list of events (NIP-33 means at most one per relay)
4. Take the first event from the result; if none, return `Ok(None)`
5. NIP-44 decrypt content; pull `device_id` from event tags (fallback to author hex)
6. Return `FetchResult { payload, updated_at, device_id }`

`fetch` always returns what's on the relay. There is no client-side dedup against a cache — that responsibility lives in the receive subscription path (Phase 3) where it actually matters.

### SyncAll (Phase 2)

The host app passes the category list; the plugin holds no schema.

```rust
pub async fn sync_all(&self, categories: &[String]) -> Result<Vec<FetchResult>>
```

1. Get signer → `Error::SignerNotSet` if absent
2. For each category in `categories`: call `fetch`
3. Collect non-None results into a `Vec<FetchResult>`
4. Returns when all fetches complete

### Receive (Phase 3)

1. On signer set + relay connect: subscribe to kind `30078` events for own pubkey, filtered to the namespace
2. Drive `client.notifications()` from a background task
3. On event received: NIP-44 decrypt, parse the d-tag to recover the category
4. Compare `created_at` with `last_seen[category]` (in-memory only)
5. If newer: update `last_seen`, emit `nostr-sync://updated` to frontend
6. If older or equal: discard

The cache is intentionally not persisted: after restart, the first event per category emits an `updated` event, which is what the host app wants for "sync on resume".

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
| `nostr-sdk` | Relay client (with built-in auto-reconnect), NIP-44, NIP-01, `NostrSigner` trait, signer storage. Re-exports `nostr` core types. |
| `serde_json` | Payload serialization |
| `tokio` | Async (Tauri's runtime) |
| `chrono` | Timestamps in IPC models |
| `uuid` | Ephemeral device ID generation |

`nostr` is not a direct dependency — `nostr-sdk` re-exports the core types. `zeroize` is not a direct dependency — `ZeroizeOnDrop` is implemented on `nostr_sdk::Keys` and fires when the `Arc<dyn NostrSigner>` inside `Client` drops.

---

## Testing Strategy

### Phase 1 — Level 1 Unit Tests (in `src/state.rs`)

All pure logic, no I/O, no network. Final shape (9 tests):

| Test | Assertion |
|---|---|
| `dtag_format_includes_namespace_category_and_version` | `build_dtag("sage", "ui-settings")` → `"sage/ui-settings/v1"` |
| `dtag_works_with_various_inputs` | dtag works across multiple namespace/category combinations |
| `namespace_rejects_slash` | namespace containing `/` → `InvalidNamespace` |
| `namespace_rejects_empty_string` | empty namespace → `InvalidNamespace` |
| `namespace_accepts_valid_identifier` | non-empty, no-slash namespaces are accepted |
| `payload_encrypt_decrypt_roundtrip` | encrypt → decrypt with same ephemeral key → original value |
| `payload_at_limit_is_accepted` | exactly 64KB → no error |
| `payload_over_limit_is_rejected` | 65KB → `PayloadTooLarge` |
| `sync_status_not_ready_without_signer` | `SyncStatus.ready == false` when signer absent |

### Phase 2 — Level 3 Tauri Command Tests

Using `tauri::test` with `MockRuntime`. Validates IPC surface: command names, argument shapes, error serialization.

### Phase 3 — Level 2 Integration + Level 4 E2E

Level 2: in-process mock relay via `tokio-tungstenite`. Tests publish/receive flow, multi-relay behavior, all-relays-down → `Err`.

Level 4: local relay (Docker), two `NostrSyncState` instances. Gated with `#[ignore = "requires local relay"]`. Runs on merge to main.

---

## Constraints Carried Forward from Spec

- Signer is never a raw `SecretKey` — stored inside `nostr_sdk::Client` as `Arc<dyn NostrSigner>`. Never cloned into plain structs.
- `clear_signer()` calls `Client::unset_signer`; when the last `Arc` reference drops, `ZeroizeOnDrop` on the underlying impl zeroes key bytes.
- `publish` returns a synchronous `Result`; transient failures are visible to the caller, not buffered in a plugin-managed queue.
- `syncAll(categories)` is never called automatically — the host app calls it explicitly with a category list after signer injection.
- d-tag format is fixed: `{namespace}/{category}/v1`.
- 64KB payload limit is global and not user-configurable.
- `device_id` is ephemeral (regenerated each process). The host app should not assume it identifies the same install across launches.
