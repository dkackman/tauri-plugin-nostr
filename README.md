# tauri-plugin-nostr-sync

Encrypted, decentralized state sync for Tauri apps using [Nostr](https://nostr.com) replaceable events as transport.

The plugin moves encrypted blobs between instances of your app via Nostr relays. Key derivation, storage, schema versioning, and conflict resolution are your app's responsibility — the plugin is transport only.

[![npm](https://img.shields.io/npm/v/tauri-plugin-nostr-sync-api)](https://www.npmjs.com/package/tauri-plugin-nostr-sync-api)
[![Crates.io Downloads (latest version)](https://img.shields.io/crates/dv/tauri-plugin-nostr-sync)](https://crates.io/crates/tauri-plugin-nostr-sync)

## Status

Pre-1.0 and under active development. **Phase 1** (Rust state machine) and **Phase 2** (Tauri IPC commands, TypeScript bindings, configurable `Builder`) are shipped. See `specs/tauri-plugin-nostr.md` for the full design.

## How it works

- State is published as [NIP-78](https://github.com/nostr-protocol/nips/blob/master/78.md) arbitrary custom app data (kind `30078`), which uses [NIP-33](https://github.com/nostr-protocol/nips/blob/master/33.md) parameterized replaceable events so relays automatically retain only the latest value per category.
- Payloads are encrypted with [NIP-44](https://github.com/nostr-protocol/nips/blob/master/44.md) before leaving the device. Plaintext never touches a relay.
- A signing identity is injected at runtime (e.g. after wallet unlock) via the `NostrSigner` trait — the plugin never holds raw key bytes. Publish calls before injection return `SignerNotSet`.
- Call `poll()` periodically to check for remote updates; new results are returned and also emitted as `nostr-sync://updated` Tauri events.

## Installation

Add the Rust crate to your `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri-plugin-nostr-sync = "0.1.0-alpha.2"
```

Add the JavaScript bindings:

```sh
pnpm add tauri-plugin-nostr-sync-api
# or
npm install tauri-plugin-nostr-sync-api
```

## Setup

Register the plugin in your Tauri app:

```rust
tauri::Builder::default()
    .plugin(
        tauri_plugin_nostr_sync::Builder::new()
            .relays(vec![
                "wss://relay.damus.io",
                "wss://relay.nostr.band",
                "wss://nos.lol",
            ])
            .app_namespace("myapp")  // prefixes all d-tags
            .build()
    )
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

`tauri_plugin_nostr_sync::init()` is a convenience alias for `Builder::new().build()` with the `"default"` namespace and no preconfigured relays.

Add the permission to your app's capability file (`src-tauri/capabilities/default.json`):

```json
{
  "permissions": ["nostr-sync:default"]
}
```

## Usage

### Signer injection

The signing identity is not provided at registration time — inject it after the user unlocks their keys (e.g. wallet unlock). The plugin accepts any type that implements `nostr_sdk::NostrSigner` and never stores raw key bytes internally.

```rust
// Derive a dedicated sync subkey from your wallet master key.
// Never pass the root wallet key — use BIP-32, HKDF, or equivalent.
let sync_secret = derive_sync_key(&wallet_master_key);
let signer = nostr_sdk::Keys::new(sync_secret);

// After unlock
app.nostr_sync().set_signer(signer).await?;

// On lock — drops the signer and zeroes key material via ZeroizeOnDrop
app.nostr_sync().clear_signer().await;
```

### TypeScript API

```typescript
import {
  publish, fetch, syncAll, poll,
  addRelay, removeRelay, getRelays,
  getPubkey, getStatus,
} from 'tauri-plugin-nostr-sync-api'

// Publish state for a named category (encrypted)
await publish('ui-settings', { theme: 'dark', fontSize: 14 })

// Publish with NIP-40 expiration (Unix timestamp seconds) — relay support is the caller's responsibility
const oneHour = Math.floor(Date.now() / 1000) + 3600
await publish('ui-settings', { theme: 'dark' }, oneHour)

// Fetch the latest known state for a category
const result = await fetch('ui-settings')
// result: { category: string, payload: unknown, updatedAt: string, deviceId: string } | null

// Fetch multiple categories at once
const results = await syncAll(['ui-settings', 'wallet'])
// results: FetchResult[]

// Poll for updates since last poll (deduplicates unchanged events)
const updates = await poll(['ui-settings', 'wallet'])
// updates: FetchResult[] — only categories with events newer than last seen

// Relay management
await addRelay('wss://relay.example.com')
await removeRelay('wss://relay.example.com')
const relays = await getRelays()
// relays: Array<{ url: string, connected: boolean, lastSeen: string | null }>

// Status
const status = await getStatus()
// status: { ready: boolean, relayCount: number, connectedRelayCount: number, deviceId: string }

// The sync pubkey (hex), or null if no signer is set
const pubkey = await getPubkey()
```

`ready` in `SyncStatus` is `true` only when a signer is set and at least one relay is connected.

### Listening for remote updates

`poll()` emits `nostr-sync://updated` for each new result as a side effect, so you can react to updates without inspecting the return value:

```typescript
import { listen } from '@tauri-apps/api/event'

// Fired by poll() for each category with a newer event than last seen
await listen('nostr-sync://updated', (event) => {
  const { category, payload, deviceId, updatedAt } = event.payload
})
```

> `nostr-sync://relay-status` and `nostr-sync://error` are planned for a future release.

### Rust API

```rust
use tauri_plugin_nostr_sync::TauriPluginNostrSyncExt;

let sync = app.nostr_sync();

sync.set_signer(signer).await?;   // impl NostrSigner + 'static
sync.clear_signer().await;        // drops signer; ZeroizeOnDrop zeroes key material

let status = sync.status().await; // SyncStatus
let pubkey = sync.pubkey().await; // Option<PublicKey>

sync.add_relay("wss://relay.example.com").await?;
sync.remove_relay("wss://relay.example.com").await?;
let relays = sync.relays().await; // Vec<RelayInfo>

sync.publish("ui-settings", &serde_json::json!({ "theme": "dark" }), None).await?;

// NIP-40: optional expiration (Unix timestamp seconds); relay support is the caller's responsibility
let expires = nostr_sdk::Timestamp::now().as_u64() + 3600;
sync.publish("ui-settings", &serde_json::json!({ "theme": "dark" }), Some(expires)).await?;
let result = sync.fetch("ui-settings").await?;   // Option<FetchResult>

let categories = vec!["ui-settings".to_string(), "wallet".to_string()];
let all = sync.sync_all(&categories).await?;     // Vec<FetchResult>
let updates = sync.poll(&categories).await?;     // Vec<FetchResult> — only new; also emits nostr-sync://updated
```

## Design notes

**Key management** — the plugin interacts with signing through the `NostrSigner` trait and never holds a raw `SecretKey`. Three rules: (1) always pass a *derived* sync keypair, not the root wallet key — use BIP-32 or HKDF to produce a dedicated identity; (2) the signer must be `ZeroizeOnDrop` so key bytes are cleared when `clear_signer` is called (`nostr_sdk::Keys` satisfies this); (3) do not downcast or clone the signer into unzeroized storage. Publish calls before `set_signer` return `SignerNotSet`.

**d-tag format** — events are keyed as `{namespace}/{category}/v1`. The namespace is set via `app_namespace` at registration and prefixes all d-tags, so multiple apps can share the same keypair without collision.

**Payload size limit** — payloads over 64KB are rejected immediately with a `PayloadTooLarge` error before any relay interaction. This matches the most conservative common relay cap.

**Startup sync** — `syncAll()` is not called automatically. Call it explicitly after signer injection to pull the latest state from relays.

**Polling vs subscribing** — `poll()` is a one-shot fetch that returns only categories with events newer than what was last seen in the current process. The deduplication state resets on restart. Call `poll()` on a timer to check for remote updates. Subscription-style push is planned for a future release.

**Conflict resolution** — last-write-wins by `created_at`. Events older than or equal to the locally known latest for a category are silently discarded.

## License

Apache License 2.0
