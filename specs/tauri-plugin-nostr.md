# tauri-plugin-nostr-sync — Plugin Specification

## Purpose

A reusable Tauri plugin that provides encrypted, decentralized state sync across multiple instances of a Tauri app using Nostr replaceable events as transport. The plugin is transport-only: it moves encrypted blobs between instances via Nostr relays. Key derivation, storage, schema versioning, and data interpretation are the responsibility of the host application.

## Intended Use Case

Any Tauri app (desktop or mobile) that needs to sync named categories of state across a user's own devices without a central server. The canonical use case is Sage wallet: syncing UI settings, wallet settings, and offers across desktop and mobile instances that share the same wallet key.

---

## Design Constraints

- **Transport only** — the plugin does not own storage, schema, or conflict resolution beyond last-write-wins
- **Encryption always on** — plaintext is never sent to a relay
- **Identity is external** — the caller derives and provides the keypair; the plugin never derives keys itself
- **Async by default** — publish is fire-and-forget with outbox retry; the caller is notified of remote changes via Tauri events
- **Mobile and desktop** — must work on iOS, Android, macOS, Windows, Linux via Tauri 2.x
- **No required infrastructure** — works with public relays out of the box; self-hosted relay is optional

---

## Nostr Specifics

| Concern | Choice |
|---|---|
| Event kind | NIP-33 parameterized replaceable events, kind `30078` |
| Encryption | NIP-44 |
| d-tag format | `{namespace}/{category}/v1` e.g. `sage/ui-settings/v1` |
| Relay protocol | Standard NIP-01 WebSocket |
| Multi-relay | Publish to all, query all, first-write-wins per relay |

Relays retain only the latest event per `(pubkey, kind, d-tag)` tuple. This is the persistence mechanism — no other storage is needed on the relay side.

---

## Rust API

### Plugin Registration

```rust
tauri::Builder::default()
    .plugin(
        tauri_plugin_nostr_sync::Builder::new()
            .relays(vec![
                "wss://relay.damus.io",
                "wss://relay.nostr.band",
                "wss://nos.lol",
            ])
            .app_namespace("sage")   // prefixes all d-tags
            .build()
    )
```

### Runtime Keypair Injection

The keypair is not provided at init time because it is typically derived from a wallet key after unlock. The plugin queues any publish attempts before the keypair is set and flushes them on injection.

```rust
// After wallet unlock, caller derives keypair and injects it
app.nostr_sync().set_keypair(secret_key: SecretKey) -> Result<()>

// Clear keypair on wallet lock
app.nostr_sync().clear_keypair() -> Result<()>
```

### State Access from Rust

```rust
app.nostr_sync().status() -> SyncStatus
app.nostr_sync().add_relay(url: &str) -> Result<()>
app.nostr_sync().remove_relay(url: &str) -> Result<()>
app.nostr_sync().relays() -> Vec<RelayInfo>
```

### Types

```rust
pub struct SyncStatus {
    pub ready: bool,              // keypair set and at least one relay connected
    pub outbox_depth: usize,      // number of events pending retry
    pub relay_count: usize,
    pub connected_relay_count: usize,
}

pub struct RelayInfo {
    pub url: String,
    pub connected: bool,
    pub last_seen: Option<DateTime<Utc>>,
}
```

---

## TypeScript API

### Commands

```typescript
import { NostrSync } from 'tauri-plugin-nostr-sync-api'

// Publish a replaceable event for a named category.
// Payload is encrypted by the plugin before sending.
// Returns immediately; delivery is handled via outbox.
await NostrSync.publish({
  category: string,       // e.g. 'ui-settings'
  payload: unknown,       // must be JSON-serializable
}): Promise<void>

// Fetch the latest known state for a category.
// Queries all connected relays and returns the most recent result.
await NostrSync.fetch({
  category: string,
}): Promise<FetchResult | null>

// Trigger a full pull sync across all registered categories.
// Useful on app resume or manual "sync now" UI action.
await NostrSync.syncAll(): Promise<void>

// Relay management
await NostrSync.addRelay({ url: string }): Promise<void>
await NostrSync.removeRelay({ url: string }): Promise<void>
await NostrSync.getRelays(): Promise<RelayInfo[]>

// Sync identity — hex-encoded pubkey, null if keypair not set
await NostrSync.getPubkey(): Promise<string | null>

// Status
await NostrSync.getStatus(): Promise<SyncStatus>
```

### Return Types

```typescript
interface FetchResult {
  payload: unknown          // decrypted, parsed JSON
  updated_at: string        // ISO 8601
  device_id: string         // origin device identifier
}

interface RelayInfo {
  url: string
  connected: boolean
  last_seen: string | null  // ISO 8601
}

interface SyncStatus {
  ready: boolean
  outbox_depth: number
  relay_count: number
  connected_relay_count: number
}
```

### Events (Rust → Frontend)

```typescript
import { listen } from '@tauri-apps/api/event'

// Fired when a remote update arrives for any category
await listen('nostr-sync://updated', (event: {
  payload: {
    category: string
    payload: unknown        // decrypted content
    device_id: string
    updated_at: string
  }
}) => { ... })

// Fired when relay connection state changes
await listen('nostr-sync://relay-status', (event: {
  payload: {
    url: string
    connected: boolean
  }
}) => { ... })

// Fired when a publish fails after all retries are exhausted
await listen('nostr-sync://error', (event: {
  payload: {
    category: string
    message: string
  }
}) => { ... })
```

---

## Internal Architecture

### Components

```
Builder
  └── configures relays, namespace, builds plugin

NostrSyncPlugin (Tauri Plugin)
  └── owns NostrSyncState as managed state

NostrSyncState
  ├── keypair: Option<SecretKey>
  ├── relay_pool: RelayPool         // nostr-sdk managed connections
  ├── outbox: OutboxQueue           // persisted retry queue
  └── namespace: String

OutboxQueue
  ├── persisted to app data dir as JSONL
  ├── retries with exponential backoff (1s, 2s, 4s... max 5min)
  └── flushed on: keypair set, relay reconnect, app startup
```

### Publish Flow

1. Caller invokes `publish({ category, payload })`
2. Plugin serializes payload to JSON
3. Plugin encrypts with NIP-44 using sync keypair
4. Plugin constructs NIP-33 event with d-tag `{namespace}/{category}/v1`
5. Plugin signs event
6. Plugin attempts publish to all connected relays simultaneously
7. On success: done
8. On any relay failure: event added to outbox with timestamp
9. Returns to caller immediately regardless of relay outcome

### Receive Flow

1. On startup and relay connect: plugin subscribes to all kind `30078` events for own pubkey
2. On event received: decrypt with NIP-44
3. Compare `created_at` with locally known latest for that category
4. If newer: emit `nostr-sync://updated` event to frontend with decrypted payload
5. If older or equal: discard

### Outbox Retry

- Queue is a persisted JSONL file in app data directory
- Each entry: `{ category, encrypted_event, attempts, last_attempt_at }`
- Retry loop runs on: startup, keypair injection, relay reconnect
- Backoff: `min(2^attempts seconds, 300s)`
- Entry removed on successful delivery to at least one relay
- Entry abandoned (logged, event emitted) after 10 attempts

---

## Crate Dependencies

| Crate | Purpose |
|---|---|
| `nostr-sdk` | Relay connections, event construction, NIP-44, signing |
| `nostr` | Core Nostr types |
| `serde` / `serde_json` | Payload serialization |
| `tokio` | Async runtime (Tauri's) |
| `chrono` | Timestamps |
| `uuid` | Device ID generation |
| `tauri` | Plugin infrastructure |

No Nostr library is used in the TypeScript layer. All Nostr logic lives in Rust.

---

## Testing Strategy

Testing this plugin requires coverage at four distinct levels. Each level has different tooling and different failure modes it catches.

### Level 1 — Unit Tests (Rust)

Pure logic tests with no I/O. Fast, no network, no Tauri runtime.

**What to test:**

- d-tag construction: `build_dtag("sage", "ui-settings")` → `"sage/ui-settings/v1"`
- Namespace sanitization: reject empty strings, slashes, special chars
- Payload round-trip: serialize → encrypt → decrypt → deserialize produces original value
- Outbox entry serialization/deserialization round-trip
- Backoff calculation: verify `2^n` capped at 300s for attempts 0–10
- `updated_at` comparison: newer wins, equal discards, older discards
- `SyncStatus` ready flag: false when no keypair, false when no relay connected, true when both

**Tooling:** Standard `#[cfg(test)]` Rust unit tests. No special setup.

```rust
#[test]
fn dtag_construction_includes_namespace_and_version() {
    assert_eq!(build_dtag("sage", "ui-settings"), "sage/ui-settings/v1");
}

#[test]
fn payload_survives_encrypt_decrypt_roundtrip() {
    let keypair = generate_test_keypair();
    let original = json!({ "theme": "dark", "font_size": 14 });
    let encrypted = encrypt_payload(&keypair, &original).unwrap();
    let decrypted = decrypt_payload(&keypair, &encrypted).unwrap();
    assert_eq!(original, decrypted);
}
```

---

### Level 2 — Integration Tests (Rust, mock relay)

Tests that exercise the full publish/receive flow against a controlled relay. No Tauri runtime needed — test the `NostrSyncState` directly.

**Approach:** Run a minimal in-process Nostr relay (or use `mockito`/`wiremock` with WebSocket support) that records received events and serves stored events on subscription.

A lightweight option: the `nostr-relay` crate or a simple `tokio-tungstenite` echo/store relay written for tests.

**What to test:**

- `publish()` sends an encrypted event to the relay
- Received event is decryptable with the same keypair
- Received event with older `created_at` is discarded
- Received event with newer `created_at` fires the update callback
- Two sequential publishes to the same category: relay retains only the latest (NIP-33 behavior)
- `syncAll()` fetches latest events for all known categories on connect
- Outbox: publish while relay is down → event queued → relay comes back → event delivered
- Outbox: after 10 failed attempts → error callback fired → entry removed from queue
- Multi-relay: publish succeeds if at least one of three relays accepts
- Multi-relay: subscribe receives event published to any relay in the pool

**Tooling:** `tokio::test`, in-process mock relay. Tests live in `tests/` directory of the crate.

```rust
#[tokio::test]
async fn publish_then_fetch_returns_same_payload() {
    let relay = MockRelay::start().await;
    let state = NostrSyncState::new(
        vec![relay.url()],
        "test",
        generate_test_keypair(),
    ).await.unwrap();

    let payload = json!({ "theme": "dark" });
    state.publish("ui-settings", &payload).await.unwrap();

    let result = state.fetch("ui-settings").await.unwrap();
    assert_eq!(result.unwrap().payload, payload);
}

#[tokio::test]
async fn older_remote_event_is_discarded() {
    // publish local with t=100, receive remote with t=50
    // assert no updated callback fired
}

#[tokio::test]
async fn outbox_retries_after_relay_reconnect() {
    // publish while relay down, assert queued
    // bring relay up, assert delivered and queue empty
}
```

---

### Level 3 — Tauri Command Tests

Tests that exercise the TypeScript-facing commands through the Tauri IPC layer using `tauri::test`.

**What to test:**

- `publish` command is accessible and returns no error with valid input
- `fetch` command returns null when no events exist for a category
- `fetch` command returns decrypted payload after a publish
- `addRelay` / `removeRelay` / `getRelays` round-trip correctly
- `getStatus` returns `ready: false` before keypair is set
- `getStatus` returns `ready: true` after keypair set and relay connected
- Events emitted: confirm `nostr-sync://updated` fires on receiving a remote event

**Tooling:** `tauri::test` module with `MockRuntime`. These tests are slower than level 2 but validate the IPC surface.

```rust
#[tauri::test]
async fn fetch_returns_null_when_no_events_exist(app: App<MockRuntime>) {
    let result: Option<FetchResult> = tauri::test::get_ipc_response(
        &app,
        tauri::webview::InvokeRequest {
            cmd: "plugin:nostr-sync|fetch".into(),
            ..Default::default()
        }
    ).await.unwrap();
    assert!(result.is_none());
}
```

---

### Level 4 — End-to-End Tests (Two Instances)

Tests that simulate two real app instances syncing with each other via a real or local relay.

**What to test:**

- Instance A publishes settings → Instance B receives `nostr-sync://updated` event
- Instance A publishes, goes offline → Instance B starts later → B fetches A's latest state
- Both instances publish simultaneously to same category → both eventually converge to the same (latest by `created_at`) value
- Instance A publishes across three relays → Instance B connected to only one of the three → B receives the event

**Tooling:**

Run a local Nostr relay for the test (e.g. `strfry` or `nostream` in Docker, or the in-process mock relay from level 2 promoted to a real TCP listener). Spawn two `NostrSyncState` instances pointing at it. These tests live in `tests/e2e/` and are gated behind a feature flag or `#[ignore]` for CI unless Docker is available.

```rust
#[tokio::test]
#[ignore = "requires local relay"]
async fn two_instances_converge_on_latest_publish() {
    let relay = LocalRelay::start_on_random_port().await;

    let instance_a = NostrSyncState::new(vec![relay.url()], "test", keypair.clone()).await?;
    let instance_b = NostrSyncState::new(vec![relay.url()], "test", keypair.clone()).await?;

    instance_a.publish("ui-settings", &json!({ "theme": "dark" })).await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = instance_b.fetch("ui-settings").await?.unwrap();
    assert_eq!(result.payload["theme"], "dark");
}
```

---

### Test Matrix Summary

| Level | Speed | Network | Tauri Runtime | What it catches |
|---|---|---|---|---|
| 1 — Unit | Very fast | None | No | Logic bugs, encoding errors |
| 2 — Integration | Fast | Mock relay | No | Relay protocol, outbox, multi-relay |
| 3 — Tauri commands | Medium | Mock relay | Yes (Mock) | IPC surface, command wiring |
| 4 — E2E | Slow | Local relay | No | Real convergence, timing, relay behavior |

CI runs levels 1–3 on every PR. Level 4 runs on merge to main (requires Docker).

---

### Additional Test Concerns

**Keypair not set:** All commands that require the keypair (`publish`, `fetch`, `syncAll`) return a typed error `SyncNotReady` when called before `set_keypair`. Tests should assert this error is returned and not a panic.

**Malformed relay response:** The mock relay should be capable of sending malformed events (bad JSON, wrong kind, wrong pubkey). The plugin must discard these silently without panicking.

**Large payloads:** Test payloads at 60KB, 100KB, and 150KB. The plugin returns a typed `PayloadTooLarge` error for events exceeding 64KB before any relay interaction. Assert the error is returned immediately and no relay connection is made.

**Clock skew:** Test behavior when the publishing device's clock is behind or ahead by up to 10 minutes. Nostr relays reject events with `created_at` too far from wall clock. The plugin should use system time and log a warning if skew is detected.

**Concurrent publishes:** Call `publish` for the same category from two concurrent tasks. Assert only one event is in-flight at a time per category (serialize per-category) and the final relay state reflects the last call.

---

## Resolved Design Decisions

**1. Outbox queue encryption at rest**

The outbox queue stores already-encrypted Nostr events (NIP-44 ciphertext). This is sufficient — the queue file contains no plaintext payload. No additional encryption layer is applied to the queue file itself, keeping the implementation simpler. The queue file should be written to the app's private data directory where OS-level permissions provide ambient protection.

**2. Payload size limit**

Hard limit of 64KB per event (the most conservative common relay cap). This is a global plugin-level limit, not per-relay. If a payload exceeds 64KB the plugin returns a typed `PayloadTooLarge` error immediately, before any relay interaction. The limit is not user-configurable — the host app is responsible for keeping payloads under this threshold. This keeps relay behavior predictable across the default relay list.

**3. Startup sync**

`syncAll()` is not called automatically. The host app calls it explicitly, typically after wallet unlock and keypair injection. This keeps the plugin decoupled from the wallet lifecycle and avoids sync attempts before the keypair is available.

**4. Sync pubkey exposure**

The plugin exposes the sync pubkey to the host app via:

```rust
app.nostr_sync().pubkey() -> Option<PublicKey>
```

```typescript
await NostrSync.getPubkey(): Promise<string | null>  // hex-encoded, null if keypair not set
```

The host app can surface this in settings UI to support relay whitelisting and user verification that all devices are using the same sync identity.

**5. Published as standalone crate**

The plugin is developed and published independently of Sage as `tauri-plugin-nostr-sync` on crates.io and `tauri-plugin-nostr-sync-api` on npm. Sage is the reference consumer. The crate should include a minimal example app in `examples/` demonstrating the full lifecycle.