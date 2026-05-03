# Phase 2 Design — tauri-plugin-nostr-sync

**Date:** 2026-05-03  
**Scope:** IPC commands, fluent Builder, TypeScript bindings, Level 2+3 tests  
**Out of scope:** receive/subscription flow, relay-status events, durable outbox (all Phase 3)

---

## Context

Phase 1 delivered `NostrSyncState` in `src/state.rs` — a complete in-process state machine covering signer lifecycle, relay management, NIP-44 encrypt/decrypt, `publish`, and `fetch`. The IPC layer (`commands.rs`) is an empty stub, `build.rs` registers zero commands, TypeScript exports nothing, and `desktop::init` hardcodes `"default"` as the namespace.

Phase 2 wires the existing state machine to the Tauri IPC surface, adds a configurable `PluginBuilder`, ships TypeScript bindings, and establishes the integration test infrastructure.

---

## Approach

**Approach B — One new file (`src/builder.rs`), one new test directory.**

- `src/builder.rs`: fluent `PluginBuilder`
- `src/commands.rs`: all 8 IPC commands
- `tests/mock_relay.rs`: reusable in-process NIP-01 relay
- `tests/integration.rs`: Level 2 tests against `NostrSyncState`
- `tests/commands.rs`: Level 3 Tauri `MockRuntime` tests

Each module has one responsibility. The mock relay is shared across test files, which matters when Level 4 tests arrive.

---

## Section 1 — Builder and Initialization

### `src/builder.rs`

A `PluginBuilder` struct (name avoids collision with `tauri::plugin::Builder`):

```rust
tauri_plugin_nostr_sync::PluginBuilder::new()
    .relays(vec!["wss://relay.damus.io", "wss://nos.lol"])
    .app_namespace("sage")
    .build()
```

Both fields default — `relays` to empty `Vec`, `namespace` to `"default"`. `build()` validates the namespace (same rules as `validate_namespace` in `state.rs`) and returns a `TauriPlugin<R>`. Invalid namespace fails at `build()` time with a panic (consistent with other Tauri builder APIs that panic on misconfiguration at startup).

The existing zero-argument `init()` in `lib.rs` stays as a convenience shortcut — it calls `PluginBuilder::new().build()` internally. Callers with no config needs don't change.

### `desktop::init()` signature change

```rust
pub fn init<R: Runtime>(
    app: &AppHandle<R>,
    relays: Vec<String>,
    namespace: &str,
) -> crate::Result<TauriPluginNostrSync<R>>
```

Creates `NostrSyncState` with the validated namespace, then spawns a `tauri::async_runtime::spawn` task to add and connect relays. `block_on` inside Tauri's already-running async runtime would panic; fire-and-forget spawn is correct since `ClientOptions::autoconnect(true)` handles connection in the background.

### Files changed

| File | Change |
|---|---|
| `src/builder.rs` | **New** — `PluginBuilder` struct |
| `src/lib.rs` | Import `builder`, update `init()` to delegate to `PluginBuilder`, wire commands |
| `src/desktop.rs` | Add `relays` and `namespace` params to `init()` |

---

## Section 2 — IPC Commands

### `src/commands.rs`

8 async commands, all taking `AppHandle<R>` and delegating to `app.nostr_sync()`:

| Command | Input model | Output |
|---|---|---|
| `publish` | `PublishRequest` | `Result<()>` |
| `fetch` | `FetchRequest` | `Result<Option<FetchResult>>` |
| `sync_all` | `SyncAllRequest` | `Result<Vec<FetchResult>>` |
| `add_relay` | inline `url: String` | `Result<()>` |
| `remove_relay` | inline `url: String` | `Result<()>` |
| `get_relays` | — | `Result<Vec<RelayInfo>>` |
| `get_pubkey` | — | `Result<Option<String>>` (hex) |
| `get_status` | — | `Result<SyncStatus>` |

`sync_all` loops sequentially over `categories`, calls `fetch` for each, and collects non-`None` results. Categories with no relay data are silently omitted from the result. No parallelism — matches the spec's intent of a simple manual pull.

### New model in `src/models.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAllRequest {
    pub categories: Vec<String>,
}
```

### `build.rs`

```rust
const COMMANDS: &[&str] = &[
    "publish", "fetch", "sync_all",
    "add_relay", "remove_relay", "get_relays",
    "get_pubkey", "get_status",
];
```

### `permissions/`

One `allow-<command>.toml` per command (8 files). `permissions/default.toml` lists all eight `allow-*` permissions as the default capability set.

### Files changed

| File | Change |
|---|---|
| `src/commands.rs` | **Replace stub** — 8 command implementations |
| `src/models.rs` | Add `SyncAllRequest` |
| `build.rs` | Populate `COMMANDS` array |
| `permissions/default.toml` | List all `allow-*` permissions |
| `permissions/allow-*.toml` | **New** — 8 permission files |

---

## Section 3 — TypeScript Bindings

### `guest-js/index.ts`

Standalone exported async functions, invoke target `plugin:tauri-plugin-nostr-sync|<command_name>`:

```typescript
export async function publish(category: string, payload: unknown): Promise<void>
export async function fetch(category: string): Promise<FetchResult | null>
export async function syncAll(categories: string[]): Promise<FetchResult[]>
export async function addRelay(url: string): Promise<void>
export async function removeRelay(url: string): Promise<void>
export async function getRelays(): Promise<RelayInfo[]>
export async function getPubkey(): Promise<string | null>
export async function getStatus(): Promise<SyncStatus>
```

Three exported interfaces matching the Rust models with camelCase fields:

```typescript
export interface FetchResult {
  payload: unknown
  updatedAt: string       // ISO 8601
  deviceId: string
}

export interface RelayInfo {
  url: string
  connected: boolean
  lastSeen: string | null // ISO 8601
}

export interface SyncStatus {
  ready: boolean
  relayCount: number
  connectedRelayCount: number
}
```

`rollup.config.js` and `package.json` are unchanged — existing pipeline handles `guest-js/ → dist-js/`.

### Files changed

| File | Change |
|---|---|
| `guest-js/index.ts` | **Replace placeholder** — 8 functions + 3 interfaces |
| `dist-js/` | Regenerated by `pnpm build` |

---

## Section 4 — Tests

### `tests/mock_relay.rs`

Minimal in-process NIP-01 relay using `tokio-tungstenite`. Listens on a random OS-assigned port.

**Storage:** `HashMap<String, Event>` keyed by d-tag — provides replaceable event semantics (kind 30078 replaces on same d-tag).

**Handles:**
- `["EVENT", event]` — store event (replace if same d-tag), reply `["OK", id, true, ""]`
- `["REQ", sub_id, filter]` — query stored events matching filter, send matching events, send `["EOSE", sub_id]`
- `["CLOSE", sub_id]` — no-op

**API:**
```rust
pub struct MockRelay { /* ... */ }
impl MockRelay {
    pub async fn start() -> Self
    pub fn url(&self) -> String   // "ws://127.0.0.1:<port>"
    pub async fn shutdown(self)
}
```

### `tests/integration.rs` — Level 2

Tests against `NostrSyncState` directly, no Tauri runtime:

| Test | What it checks |
|---|---|
| `publish_then_fetch_returns_same_payload` | Full round-trip through mock relay |
| `fetch_returns_none_when_no_events_exist` | Empty relay returns `None` |
| `publish_without_signer_returns_signer_not_set` | Pre-signer error surface |
| `fetch_without_signer_returns_signer_not_set` | Pre-signer error surface |
| `payload_at_64kb_limit_accepted` | Boundary condition |
| `payload_over_64kb_limit_rejected` | `PayloadTooLarge` error |
| `sync_all_returns_all_fetched_categories` | Multi-category fetch |
| `sync_all_omits_categories_with_no_data` | Sparse result handling |
| `publish_with_relay_down_returns_err` | All-relays-unreachable error surface |

### `tests/commands.rs` — Level 3

Tests via `tauri::test` with `MockRuntime`. Intentionally narrower than Level 2 — validates IPC wiring, not logic:

| Test | What it checks |
|---|---|
| `fetch_returns_null_when_no_events_exist` | Command reachable, null response deserialization |
| `get_status_returns_not_ready_before_signer` | `getStatus` IPC surface |
| `get_relays_returns_empty_list_initially` | `getRelays` IPC surface |

### `Cargo.toml` dev-dependencies added

```toml
[dev-dependencies]
tokio-tungstenite = "0.26"   # already a transitive dep via nostr-sdk; ws:// only, no TLS needed
tauri = { version = "2", features = ["test"] }
serde_json = "1"
```

---

## Build Sequence

1. `src/builder.rs` — new file, no dependencies on later steps
2. `src/desktop.rs` — update `init()` signature
3. `src/lib.rs` — import builder, update `init()`, wire generate_handler
4. `src/models.rs` — add `SyncAllRequest`
5. `src/commands.rs` — implement all 8 commands
6. `build.rs` + `permissions/` — register commands and permissions
7. `guest-js/index.ts` — TypeScript bindings, then `pnpm build`
8. `tests/` — mock relay first, then integration, then commands

---

## What Is Explicitly Out of Scope

- Background subscription loop (`Client::notifications()`) — Phase 3
- `nostr-sync://updated`, `nostr-sync://relay-status`, `nostr-sync://error` events — Phase 3
- Durable outbox / retry queue — resolved as "not in plugin" per spec
- Mobile implementation — mobile stub remains unchanged
- Level 4 E2E tests — require Docker, deferred
