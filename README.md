# tauri-plugin-nostr

Encrypted, decentralized state sync for Tauri apps using [Nostr](https://nostr.com) replaceable events as transport.

The plugin moves encrypted blobs between instances of your app via Nostr relays. Key derivation, storage, schema versioning, and conflict resolution are your app's responsibility — the plugin is transport only.

## How it works

- State is published as [NIP-33](https://github.com/nostr-protocol/nips/blob/master/33.md) parameterized replaceable events (kind `30078`) so relays automatically retain only the latest value per category.
- Payloads are encrypted with [NIP-44](https://github.com/nostr-protocol/nips/blob/master/44.md) before leaving the device. Plaintext never touches a relay.
- The keypair is injected at runtime (e.g. after wallet unlock), not at init time. Publish calls made before injection are queued and flushed automatically.
- Publish is fire-and-forget. Failed deliveries are retried via a persisted outbox with exponential backoff.
- Incoming events are delivered to your frontend via Tauri events.

## Installation

Add the Rust crate to your `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri-plugin-nostr = "0.1"
```

Add the JavaScript bindings:

```sh
pnpm add tauri-plugin-nostr-api
# or
npm install tauri-plugin-nostr-api
```

## Setup

Register the plugin in your Tauri app:

```rust
tauri::Builder::default()
    .plugin(
        tauri_plugin_nostr::Builder::new()
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

Add the permission to your app's capability file (`src-tauri/capabilities/default.json`):

```json
{
  "permissions": ["tauri-plugin-nostr:default"]
}
```

## Usage

### Keypair injection

The keypair is not provided at registration time — inject it after the user unlocks their keys (e.g. wallet unlock):

```rust
// After unlock
app.nostr_sync().set_keypair(secret_key)?;

// On lock
app.nostr_sync().clear_keypair()?;
```

### TypeScript API

```typescript
import { NostrSync } from 'tauri-plugin-nostr-api'

// Publish state for a named category (encrypted, fire-and-forget)
await NostrSync.publish({
  category: 'ui-settings',
  payload: { theme: 'dark', fontSize: 14 },
})

// Fetch the latest known state for a category
const result = await NostrSync.fetch({ category: 'ui-settings' })
// result: { payload: unknown, updated_at: string, device_id: string } | null

// Pull latest state for all known categories
await NostrSync.syncAll()

// Relay management
await NostrSync.addRelay({ url: 'wss://relay.example.com' })
await NostrSync.removeRelay({ url: 'wss://relay.example.com' })
const relays = await NostrSync.getRelays()
// relays: Array<{ url: string, connected: boolean, last_seen: string | null }>

// Status
const status = await NostrSync.getStatus()
// status: { ready: boolean, outbox_depth: number, relay_count: number, connected_relay_count: number }

// The sync pubkey (hex), or null if no keypair is set
const pubkey = await NostrSync.getPubkey()
```

`ready` in `SyncStatus` is `true` only when a keypair is set and at least one relay is connected.

### Listening for remote updates

```typescript
import { listen } from '@tauri-apps/api/event'

// Fired when a remote update arrives for any category
await listen('nostr-sync://updated', (event) => {
  const { category, payload, device_id, updated_at } = event.payload
})

// Fired when relay connection state changes
await listen('nostr-sync://relay-status', (event) => {
  const { url, connected } = event.payload
})

// Fired when a publish fails after all retries are exhausted
await listen('nostr-sync://error', (event) => {
  const { category, message } = event.payload
})
```

### Rust API

```rust
use tauri_plugin_nostr::TauriPluginNostrExt;

let sync = app.nostr_sync();

sync.set_keypair(secret_key)?;
sync.clear_keypair()?;

let status = sync.status();
let pubkey = sync.pubkey(); // Option<PublicKey>

sync.add_relay("wss://relay.example.com")?;
sync.remove_relay("wss://relay.example.com")?;
let relays = sync.relays(); // Vec<RelayInfo>
```

## Design notes

**d-tag format** — events are keyed as `{namespace}/{category}/v1`. The namespace is set via `app_namespace` at registration and prefixes all d-tags, so multiple apps can share the same keypair without collision.

**Payload size limit** — payloads over 64KB are rejected immediately with a `PayloadTooLarge` error before any relay interaction. This matches the most conservative common relay cap.

**Startup sync** — `syncAll()` is not called automatically. Call it explicitly after keypair injection to pull the latest state from relays.

**Outbox persistence** — the retry queue is written to the app's private data directory as a JSONL file containing already-encrypted ciphertext. Retries use exponential backoff (2^n seconds, capped at 5 minutes). After 10 failed attempts the entry is dropped and a `nostr-sync://error` event is emitted.

**Conflict resolution** — last-write-wins by `created_at`. Incoming events older than or equal to the locally known latest for a category are silently discarded.

## License

MIT
