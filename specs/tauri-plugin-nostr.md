# tauri-plugin-nostr-sync — Plugin Specification

## Purpose

A reusable Tauri plugin that provides encrypted, decentralized state sync across multiple instances of a Tauri app using Nostr replaceable events as transport. The plugin is transport-only: it moves encrypted blobs between instances via Nostr relays. Key derivation, storage, schema versioning, and data interpretation are the responsibility of the host application.

## Intended Use Case

Any Tauri app (desktop or mobile) that needs to sync named categories of state across a user's own devices without a central server. The canonical use case is Sage wallet: syncing UI settings, wallet settings, and offers across desktop and mobile instances that share the same wallet key.

---

## Design Constraints

- **Transport only** — the plugin does not own storage, schema, or conflict resolution beyond last-write-wins
- **Encryption always on** — plaintext is never sent to a relay
- **Identity is external** — the caller derives and provides the signing identity; the plugin never derives keys itself
- **Secret key never retained** — the plugin holds signing capability through a `NostrSigner` trait object, not raw key bytes; when cleared, the trait object is dropped immediately and key material is zeroed by the signer's `ZeroizeOnDrop` impl
- **Derived sync key only** — callers MUST derive a Nostr-specific subkey from the wallet master key before passing it to the plugin; passing the root wallet key is explicitly prohibited
- **Synchronous publish result** — `publish` returns a `Result` to the caller; errors (no relay accepted, signer not set, payload too large, encrypt failure) surface immediately. The caller is notified of remote changes via Tauri events.
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

### Runtime Signer Injection

The signing identity is not provided at init time because it is typically derived from a wallet key after unlock. The plugin queues any publish attempts before the signer is set and flushes them on injection.

```rust
// After wallet unlock, caller provides a signer implementation
app.nostr_sync().set_signer(signer: impl NostrSigner + Send + Sync + 'static) -> Result<()>

// Clear signer on wallet lock — drops the trait object immediately; ZeroizeOnDrop zeroes key material
app.nostr_sync().clear_signer() -> Result<()>
```

`NostrSigner` is the trait from `nostr-sdk`. The simplest implementation wraps a `Keys` value:

```rust
// Derive a Nostr-specific subkey from the wallet master key — never pass the root key
let sync_secret = derive_sync_key(&wallet_master_key);   // caller's responsibility
let signer = nostr_sdk::Keys::new(sync_secret);
app.nostr_sync().set_signer(signer)?;
```

**Key derivation requirement** — the sync keypair MUST be a deterministically derived child key of the wallet master key, not the root key itself. Use BIP-32, HKDF, or another scheme your app already employs. The public half of this key is the sync identity visible on relays; the private half is used solely for NIP-44 encryption and NIP-01 event signing within this plugin.

**Zeroization** — `nostr_sdk::Keys` implements `ZeroizeOnDrop`. The plugin must not clone the signer or the underlying `SecretKey` into plain structs; doing so bypasses zeroing and leaves key bytes in heap memory after `clear_signer` returns.

### State Access from Rust

```rust
app.nostr_sync().status() -> SyncStatus
app.nostr_sync().pubkey() -> Option<PublicKey>
app.nostr_sync().add_relay(url: &str) -> Result<()>
app.nostr_sync().remove_relay(url: &str) -> Result<()>
app.nostr_sync().relays() -> Vec<RelayInfo>
```

### Types

```rust
pub struct SyncStatus {
    pub ready: bool,              // signer set and at least one relay connected
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
// Returns when send_event completes; rejects if no relay accepted.
await NostrSync.publish({
  category: string,       // e.g. 'ui-settings'
  payload: unknown,       // must be JSON-serializable
}): Promise<void>

// Fetch the latest known state for a category.
// Queries all connected relays and returns the most recent result.
// Always returns what's on the relay; no client-side dedup.
await NostrSync.fetch({
  category: string,
}): Promise<FetchResult | null>

// Trigger a full pull sync across the categories the host app cares about.
// The host owns the category list; the plugin holds no schema.
await NostrSync.syncAll({
  categories: string[],
}): Promise<FetchResult[]>

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

// Fired when an out-of-band error occurs (e.g. malformed incoming event,
// decryption failure on a received event). Synchronous publish errors are
// returned directly from the publish call and do NOT use this channel.
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
  ├── client: nostr_sdk::Client   // owns relay pool AND signer slot
  ├── namespace: String
  └── device_id: String           // ephemeral, regenerated each process

(Signer is held inside Client via Client::set_signer/unset_signer/signer.
 The plugin does not maintain a parallel signer field.)
```

The plugin relies on `nostr_sdk::Client` for relay pool management, signer storage, automatic reconnect (with exponential backoff via `RelayOptions::retry_interval` / `adjust_retry_interval`), and event delivery via `Client::notifications()`. The plugin adds: namespace + d-tag contract, NIP-44 encrypt/decrypt to self, payload size limit, and Tauri event bridging.

### Publish Flow

1. Caller invokes `publish({ category, payload })`
2. Plugin retrieves the signer from the client → `Error::SignerNotSet` if absent
3. Plugin serializes payload to JSON; rejects if > 64KB (`Error::PayloadTooLarge`)
4. Plugin NIP-44-encrypts the JSON to its own pubkey
5. Plugin constructs NIP-33 event with d-tag `{namespace}/{category}/v1` and a `device_id` tag
6. Plugin signs the event via the signer
7. Plugin calls `client.send_event(event)`; the SDK broadcasts to all WRITE relays
8. If `Output.success` is non-empty (at least one relay accepted) → return `Ok(())`
9. If zero relays accepted → return `Err`; the caller decides what to do

There is no plugin-managed retry queue. Transient relay drops are handled by `nostr-sdk`'s built-in reconnect. Durable "publish-while-offline" is a host-app concern; the plugin surfaces failures synchronously so the host can re-call `publish` at its discretion.

### Receive Flow (Phase 3)

1. On startup and relay connect: plugin subscribes to kind `30078` events authored by own pubkey, filtered to the configured namespace
2. On event received via `Client::notifications()`: NIP-44 decrypt
3. Compare `created_at` with the in-memory last-seen timestamp for that category
4. If newer: update the in-memory timestamp, emit `nostr-sync://updated` to the frontend with the decrypted payload
5. If older or equal: discard

The last-seen timestamp cache is in-memory only; it does not persist across restarts. After a restart, the first event received per category will always emit `nostr-sync://updated`, which is the correct behavior for "sync on app resume".

---

## Crate Dependencies

| Crate | Purpose |
|---|---|
| `nostr-sdk` | Relay connections (with built-in auto-reconnect), event construction, NIP-44, signing, `NostrSigner` trait, signer storage |
| `serde` / `serde_json` | Payload serialization |
| `tokio` | Async runtime (Tauri's) |
| `chrono` | Timestamps in IPC models |
| `uuid` | Ephemeral device ID generation |
| `tauri` | Plugin infrastructure |

`nostr-sdk` re-exports the core `nostr` types (`Event`, `Keys`, `Timestamp`, etc.); a direct dependency on `nostr` is not needed. `zeroize` is also not a direct dependency — `ZeroizeOnDrop` lives on `nostr_sdk::Keys` and fires automatically when the `Arc<dyn NostrSigner>` inside `Client` is dropped.

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
- Payload size: at-limit (64KB) accepted, over-limit rejected
- `SyncStatus` ready flag: false when no signer, false when no relay connected, true when both

**Tooling:** Standard `#[cfg(test)]` Rust unit tests. No special setup.

```rust
#[test]
fn dtag_construction_includes_namespace_and_version() {
    assert_eq!(build_dtag("sage", "ui-settings"), "sage/ui-settings/v1");
}

#[test]
fn payload_survives_encrypt_decrypt_roundtrip() {
    let keys = nostr_sdk::Keys::generate();   // ephemeral; never the wallet root key
    let original = json!({ "theme": "dark", "font_size": 14 });
    let encrypted = encrypt_payload(&keys, &original).unwrap();
    let decrypted = decrypt_payload(&keys, &encrypted).unwrap();
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
- Received event with older `created_at` is discarded by the receive subscription
- Received event with newer `created_at` fires the update callback
- Two sequential publishes to the same category: relay retains only the latest (NIP-33 behavior)
- `syncAll(categories)` fetches latest events for the supplied list and returns them
- Publish with all relays unreachable → returns `Err`; the caller can retry at its discretion
- Multi-relay: publish succeeds if at least one of three relays accepts
- Multi-relay: subscribe receives event published to any relay in the pool

**Tooling:** `tokio::test`, in-process mock relay. Tests live in `tests/` directory of the crate.

```rust
#[tokio::test]
async fn publish_then_fetch_returns_same_payload() {
    let relay = MockRelay::start().await;
    let keys = nostr_sdk::Keys::generate();   // ephemeral test signer
    let state = NostrSyncState::new(vec![relay.url()], "test").await.unwrap();
    state.set_signer(keys).await.unwrap();

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
async fn publish_with_all_relays_down_returns_err() {
    // start state with one relay url, do not start the relay
    // call publish, assert Err is returned synchronously
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

    let keys = nostr_sdk::Keys::generate();   // shared sync identity for both instances
    let instance_a = NostrSyncState::new(vec![relay.url()], "test").await?;
    instance_a.set_signer(keys.clone()).await?;
    let instance_b = NostrSyncState::new(vec![relay.url()], "test").await?;
    instance_b.set_signer(keys).await?;

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
| 2 — Integration | Fast | Mock relay | No | Relay protocol, multi-relay, error surface |
| 3 — Tauri commands | Medium | Mock relay | Yes (Mock) | IPC surface, command wiring |
| 4 — E2E | Slow | Local relay | No | Real convergence, timing, relay behavior |

CI runs levels 1–3 on every PR. Level 4 runs on merge to main (requires Docker).

---

### Additional Test Concerns

**Signer not set:** All commands that require a signing identity (`publish`, `fetch`, `syncAll`) return a typed error `SyncNotReady` when called before `set_signer`. Tests should assert this error is returned and not a panic.

**Malformed relay response:** The mock relay should be capable of sending malformed events (bad JSON, wrong kind, wrong pubkey). The plugin must discard these silently without panicking.

**Large payloads:** Test payloads at 60KB, 100KB, and 150KB. The plugin returns a typed `PayloadTooLarge` error for events exceeding 64KB before any relay interaction. Assert the error is returned immediately and no relay connection is made.

**Clock skew:** Test behavior when the publishing device's clock is behind or ahead by up to 10 minutes. Nostr relays reject events with `created_at` too far from wall clock. The plugin should use system time and log a warning if skew is detected.

**Concurrent publishes:** Call `publish` for the same category from two concurrent tasks. Assert only one event is in-flight at a time per category (serialize per-category) and the final relay state reflects the last call.

---

## Resolved Design Decisions

**1. No durable outbox**

The plugin does not maintain a persisted retry queue. Three reasons:

*nostr-sdk already auto-reconnects.* `RelayOptions` defaults to `reconnect=true`, `retry_interval=10s`, `adjust_retry_interval=true`. Transient relay drops within a session are handled without plugin involvement.

*The error is observable on the spot.* `Client::send_event` returns `Output<EventId>` with `success` and `failed` sets. The plugin's `publish` returns `Err` when zero relays accepted; the caller decides whether to retry, surface a UI error, or queue at the application layer.

*Durable cross-restart publish is a host concern.* If the host app needs "publish-while-offline → retransmit on next launch", it can layer that on top of `publish` using its own persistence — keeping the plugin transport-only and small. The plugin can later add an `Outbox` trait if a clear shared use case emerges.

**2. Payload size limit**

Hard limit of 64KB per event (the most conservative common relay cap). This is a global plugin-level limit, not per-relay. If a payload exceeds 64KB the plugin returns a typed `PayloadTooLarge` error immediately, before any relay interaction. The limit is not user-configurable — the host app is responsible for keeping payloads under this threshold. This keeps relay behavior predictable across the default relay list.

**3. Startup sync**

`syncAll(categories)` is not called automatically. The host app calls it explicitly, typically after wallet unlock and signer injection, and supplies the list of categories it cares about. This keeps the plugin decoupled from both the wallet lifecycle and the host app's schema — the plugin holds no list of "known categories", which keeps state from drifting between sessions.

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

**6. Secret key injection security**

Three concerns drove the `NostrSigner` trait design instead of accepting a raw `SecretKey`:

*Do not pass the root wallet key.* The plugin uses a secp256k1 keypair for NIP-44 encryption and NIP-01 signing. If the caller passes the wallet's root private key, any bug in the plugin (relay handler, future IPC surface) becomes a wallet-draining vulnerability. Callers MUST derive a dedicated sync keypair from the wallet master key. The derivation is the caller's responsibility; BIP-32 child key derivation or HKDF are both acceptable. The derived public key is the sync identity visible on relays; it can safely be shown in settings UI.

*The plugin does not hold raw key bytes itself.* The signer lives inside `nostr_sdk::Client` (set via `Client::set_signer`), accessed as `Arc<dyn NostrSigner>`. The plugin never stores a `SecretKey` field of its own. A compromised plugin cannot trivially exfiltrate the key — it can only invoke signing operations, which are atomic and auditable.

*Zeroize on clear.* When `clear_signer` is called, the plugin invokes `Client::unset_signer`. The `Arc<dyn NostrSigner>` inside the client drops; the underlying implementation (e.g. `nostr_sdk::Keys`) implements `ZeroizeOnDrop` so the key bytes are zeroed before the allocator reclaims the memory. Implementations that wrap `SecretKey` directly satisfy this if they do not clone the key into unzeroized storage. The plugin MUST NOT clone the signer or access the inner `SecretKey` through any downcast.