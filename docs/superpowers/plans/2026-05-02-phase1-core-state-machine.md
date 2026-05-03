# tauri-plugin-nostr-sync Phase 1: Core State Machine

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and unit-test `NostrSyncState` — the core Nostr sync engine with relay management, runtime signer injection, NIP-44 encrypt/decrypt, and fire-and-forget publish/fetch — entirely in pure Rust with no Tauri IPC wiring.

**Architecture:** `NostrSyncState` wraps `nostr_sdk::Client` for relay connections and holds the signer behind `tokio::sync::RwLock<Option<Arc<dyn NostrSigner>>>` for runtime injection. Events are built with `EventBuilder`, signed asynchronously via the signer trait, then broadcast via `client.send_event`. The `Client` carries no signer of its own. `TauriPluginNostrSync<R>` in `desktop.rs` wraps `Arc<NostrSyncState>` and exposes the host-app-facing Rust API.

**Tech Stack:** Rust 1.77+, nostr-sdk 0.38, tokio (Tauri's runtime), serde_json 1, uuid 1, chrono 0.4, tauri 2.5, thiserror 2

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `Cargo.toml` | Modify | Rename crate, add nostr-sdk/serde_json/uuid/chrono |
| `build.rs` | Modify | Update plugin name string |
| `src/lib.rs` | Modify | Rename plugin ID and ext trait |
| `src/error.rs` | Modify | Add SignerNotSet, PayloadTooLarge, EncryptionFailed, DecryptionFailed, InvalidNamespace, Nostr variants |
| `src/models.rs` | Modify | Replace ping types with SyncStatus, RelayInfo, FetchResult, PublishRequest, FetchRequest |
| `src/outbox.rs` | Create | `pub fn backoff_secs(attempts: u32) -> u64` only — no file I/O |
| `src/state.rs` | Create | `NostrSyncState`, `build_dtag`, `validate_namespace`, all sync methods |
| `src/desktop.rs` | Modify | `TauriPluginNostrSync<R>` wrapping `Arc<NostrSyncState>` |
| `src/commands.rs` | Modify | Remove ping, leave empty (commands come in Phase 2) |

---

## Task 1: Rename crate and add dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `build.rs`
- Modify: `src/lib.rs`

- [ ] **Replace `Cargo.toml` with the renamed + expanded version:**

```toml
[workspace]
members = ["."]
exclude = ["examples"]

[package]
name = "tauri-plugin-nostr-sync"
version = "0.1.0"
authors = ["You"]
description = "Encrypted decentralized state sync via Nostr for Tauri apps"
edition = "2021"
rust-version = "1.77.2"
exclude = ["/examples", "/dist-js", "/guest-js", "/node_modules"]
links = "tauri-plugin-nostr-sync"

[dependencies]
tauri = { version = "2.5.0" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2"
nostr-sdk = "0.38"
tokio = { version = "1", features = ["sync"] }
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }

[build-dependencies]
tauri-plugin = { version = "2.2.0", features = ["build"] }
```

- [ ] **Update `build.rs` to use the new plugin name:**

```rust
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
```

- [ ] **Update `src/lib.rs` to rename plugin ID and ext trait:**

```rust
use std::sync::Arc;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;
mod state;
pub(crate) mod outbox;

pub use error::{Error, Result};
pub use state::NostrSyncState;

#[cfg(desktop)]
use desktop::TauriPluginNostrSync;
#[cfg(mobile)]
use mobile::TauriPluginNostrSync;

pub trait TauriPluginNostrSyncExt<R: Runtime> {
    fn nostr_sync(&self) -> &TauriPluginNostrSync<R>;
}

impl<R: Runtime, T: Manager<R>> TauriPluginNostrSyncExt<R> for T {
    fn nostr_sync(&self) -> &TauriPluginNostrSync<R> {
        self.state::<TauriPluginNostrSync<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("tauri-plugin-nostr-sync")
        .invoke_handler(tauri::generate_handler![])
        .setup(|app, _api| {
            #[cfg(desktop)]
            {
                let plugin = desktop::init(app)?;
                app.manage(plugin);
            }
            Ok(())
        })
        .build()
}
```

- [ ] **Replace `src/commands.rs` with an empty stub:**

```rust
// Commands will be added in Phase 2.
```

- [ ] **Run `cargo check` and fix any errors:**

```bash
cargo check
```

Expected: compiles cleanly (no warnings about unused imports are OK for now).

- [ ] **Commit:**

```bash
git add Cargo.toml build.rs src/lib.rs src/commands.rs
git commit -m "chore: rename plugin to tauri-plugin-nostr-sync, add nostr-sdk deps"
```

---

## Task 2: Expand error enum

**Files:**
- Modify: `src/error.rs`

- [ ] **Replace `src/error.rs`:**

```rust
use serde::{ser::Serializer, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("nostr error: {0}")]
    Nostr(#[from] nostr_sdk::client::Error),

    #[error("signer not set — call set_signer before publishing or fetching")]
    SignerNotSet,

    #[error("payload too large: {size} bytes exceeds {limit} byte limit")]
    PayloadTooLarge { size: usize, limit: usize },

    #[error("encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("invalid namespace '{0}': must be non-empty and contain no '/' characters")]
    InvalidNamespace(String),

    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
```

- [ ] **Run `cargo check`:**

```bash
cargo check
```

Expected: clean compile.

- [ ] **Commit:**

```bash
git add src/error.rs
git commit -m "feat: expand error enum with sync-specific variants"
```

---

## Task 3: Define IPC models

**Files:**
- Modify: `src/models.rs`

- [ ] **Replace `src/models.rs` with the real types:**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishRequest {
    pub category: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchRequest {
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    pub payload: Value,
    pub updated_at: DateTime<Utc>,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub ready: bool,
    pub outbox_depth: usize,
    pub relay_count: usize,
    pub connected_relay_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayInfo {
    pub url: String,
    pub connected: bool,
    pub last_seen: Option<DateTime<Utc>>,
}
```

- [ ] **Run `cargo check`:**

```bash
cargo check
```

Expected: clean compile.

- [ ] **Commit:**

```bash
git add src/models.rs
git commit -m "feat: replace ping models with real IPC types"
```

---

## Task 4: Backoff helper with unit test (TDD)

**Files:**
- Create: `src/outbox.rs`

The backoff function is pure logic — no I/O. The `OutboxQueue` struct with file persistence comes in Phase 3. Define the function here now so it can be unit-tested.

- [ ] **Create `src/outbox.rs` with a stub that panics:**

```rust
/// Returns the number of seconds to wait before the next retry attempt.
/// Implements capped exponential backoff: min(2^attempts, 300).
pub fn backoff_secs(attempts: u32) -> u64 {
    todo!("implement backoff_secs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_exponential_up_to_cap() {
        assert_eq!(backoff_secs(0), 1);
        assert_eq!(backoff_secs(1), 2);
        assert_eq!(backoff_secs(2), 4);
        assert_eq!(backoff_secs(3), 8);
        assert_eq!(backoff_secs(4), 16);
        assert_eq!(backoff_secs(5), 32);
        assert_eq!(backoff_secs(6), 64);
        assert_eq!(backoff_secs(7), 128);
        assert_eq!(backoff_secs(8), 256);
        assert_eq!(backoff_secs(9), 300);  // capped
        assert_eq!(backoff_secs(10), 300); // still capped
        assert_eq!(backoff_secs(20), 300); // still capped
    }
}
```

- [ ] **Run the test and confirm it panics (todo! fires):**

```bash
cargo test backoff_is_exponential_up_to_cap
```

Expected: `FAILED` with `not yet implemented: implement backoff_secs`.

- [ ] **Implement `backoff_secs`:**

```rust
pub fn backoff_secs(attempts: u32) -> u64 {
    let base: u64 = 2u64.saturating_pow(attempts);
    base.min(300)
}
```

- [ ] **Run the test and confirm it passes:**

```bash
cargo test backoff_is_exponential_up_to_cap
```

Expected: `test outbox::tests::backoff_is_exponential_up_to_cap ... ok`.

- [ ] **Commit:**

```bash
git add src/outbox.rs
git commit -m "feat: add backoff_secs helper with unit test"
```

---

## Task 5: d-tag construction and namespace validation (TDD)

**Files:**
- Create: `src/state.rs`

- [ ] **Create `src/state.rs` with stub functions:**

```rust
use crate::{Error, Result};

/// Constructs the NIP-33 d-tag value: `{namespace}/{category}/v1`
pub(crate) fn build_dtag(namespace: &str, category: &str) -> String {
    todo!("implement build_dtag")
}

/// Validates that a namespace is non-empty and contains no '/' characters.
pub(crate) fn validate_namespace(namespace: &str) -> Result<()> {
    todo!("implement validate_namespace")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtag_format_includes_namespace_category_and_version() {
        assert_eq!(build_dtag("sage", "ui-settings"), "sage/ui-settings/v1");
    }

    #[test]
    fn dtag_works_with_various_inputs() {
        assert_eq!(build_dtag("myapp", "wallet-config"), "myapp/wallet-config/v1");
    }

    #[test]
    fn namespace_rejects_empty_string() {
        assert!(matches!(
            validate_namespace(""),
            Err(Error::InvalidNamespace(_))
        ));
    }

    #[test]
    fn namespace_rejects_slash() {
        assert!(matches!(
            validate_namespace("bad/namespace"),
            Err(Error::InvalidNamespace(_))
        ));
    }

    #[test]
    fn namespace_accepts_valid_identifier() {
        assert!(validate_namespace("sage").is_ok());
        assert!(validate_namespace("my-app").is_ok());
        assert!(validate_namespace("app_v2").is_ok());
    }
}
```

- [ ] **Run tests and confirm they panic:**

```bash
cargo test dtag_format
cargo test namespace_rejects
cargo test namespace_accepts
```

Expected: all `FAILED` with `not yet implemented`.

- [ ] **Implement both functions:**

```rust
pub(crate) fn build_dtag(namespace: &str, category: &str) -> String {
    format!("{}/{}/v1", namespace, category)
}

pub(crate) fn validate_namespace(namespace: &str) -> Result<()> {
    if namespace.is_empty() {
        return Err(Error::InvalidNamespace(namespace.to_string()));
    }
    if namespace.contains('/') {
        return Err(Error::InvalidNamespace(namespace.to_string()));
    }
    Ok(())
}
```

- [ ] **Run tests and confirm they all pass:**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Commit:**

```bash
git add src/state.rs
git commit -m "feat: add build_dtag and validate_namespace with unit tests"
```

---

## Task 6: NostrSyncState struct, constructor, and status

**Files:**
- Modify: `src/state.rs`

- [ ] **Add imports and the struct definition to `src/state.rs`** (append after the existing functions):

```rust
use std::collections::HashMap;
use std::sync::Arc;

use nostr_sdk::{Client, NostrSigner, PublicKey, RelayStatus, Timestamp};
use tokio::sync::RwLock;

use crate::{RelayInfo, SyncStatus};

pub struct NostrSyncState {
    pub(crate) namespace: String,
    pub(crate) device_id: String,
    pub(crate) client: Client,
    pub(crate) signer: RwLock<Option<Arc<dyn NostrSigner + Send + Sync>>>,
    pub(crate) known_timestamps: RwLock<HashMap<String, Timestamp>>,
}

impl NostrSyncState {
    pub fn new(namespace: &str) -> Result<Self> {
        validate_namespace(namespace)?;
        Ok(Self {
            namespace: namespace.to_string(),
            device_id: uuid::Uuid::new_v4().to_string(),
            client: Client::new(),
            signer: RwLock::new(None),
            known_timestamps: RwLock::new(HashMap::new()),
        })
    }

    pub async fn status(&self) -> SyncStatus {
        let has_signer = self.signer.read().await.is_some();
        let relays_map = self.client.relays().await;
        let relay_count = relays_map.len();
        let connected_relay_count = relays_map
            .values()
            .filter(|r| matches!(r.status(), RelayStatus::Connected))
            .count();

        SyncStatus {
            ready: has_signer && connected_relay_count > 0,
            outbox_depth: 0, // OutboxQueue added in Phase 3
            relay_count,
            connected_relay_count,
        }
    }
}
```

- [ ] **Add the status test to the `#[cfg(test)]` block in `src/state.rs`:**

```rust
    #[tokio::test]
    async fn sync_status_not_ready_without_signer() {
        let state = NostrSyncState::new("testapp").unwrap();
        let status = state.status().await;
        assert!(!status.ready);
        assert_eq!(status.outbox_depth, 0);
    }
```

- [ ] **Run the test:**

```bash
cargo test sync_status_not_ready
```

Expected: `test state::tests::sync_status_not_ready_without_signer ... ok`.

- [ ] **Run `cargo check` to confirm no other breakage:**

```bash
cargo check
```

- [ ] **Commit:**

```bash
git add src/state.rs
git commit -m "feat: add NostrSyncState struct, new(), and status() with test"
```

---

## Task 7: Signer management (set_signer, clear_signer, pubkey)

**Files:**
- Modify: `src/state.rs`

- [ ] **Add signer methods to the `impl NostrSyncState` block:**

```rust
    pub async fn set_signer(&self, signer: impl NostrSigner + Send + Sync + 'static) -> Result<()> {
        let mut guard = self.signer.write().await;
        *guard = Some(Arc::new(signer));
        Ok(())
    }

    pub async fn clear_signer(&self) {
        let mut guard = self.signer.write().await;
        *guard = None;
        // The Arc drops here; ZeroizeOnDrop on the underlying Keys zeroes key bytes.
    }

    pub async fn pubkey(&self) -> Option<PublicKey> {
        let guard = self.signer.read().await;
        let signer = guard.as_ref()?;
        signer.public_key().await.ok()
    }
```

- [ ] **Run `cargo check`:**

```bash
cargo check
```

Expected: clean compile.

- [ ] **Commit:**

```bash
git add src/state.rs
git commit -m "feat: add set_signer, clear_signer, pubkey to NostrSyncState"
```

---

## Task 8: Relay management (add_relay, remove_relay, relays)

**Files:**
- Modify: `src/state.rs`

- [ ] **Add relay methods to the `impl NostrSyncState` block:**

```rust
    pub async fn add_relay(&self, url: &str) -> Result<()> {
        self.client.add_relay(url).await?;
        self.client.connect_relay(url).await?;
        Ok(())
    }

    pub async fn remove_relay(&self, url: &str) -> Result<()> {
        self.client.remove_relay(url).await?;
        Ok(())
    }

    pub async fn relays(&self) -> Vec<RelayInfo> {
        let relays_map = self.client.relays().await;
        relays_map
            .into_iter()
            .map(|(url, relay)| RelayInfo {
                url: url.to_string(),
                connected: matches!(relay.status(), RelayStatus::Connected),
                last_seen: None, // Phase 3: track last_seen via subscription events
            })
            .collect()
    }
```

Note: `connect_relay` may be named `connect` in some nostr-sdk versions. Run `cargo doc --open` after adding the dependency and check `Client`'s method list if this doesn't compile.

- [ ] **Run `cargo check`:**

```bash
cargo check
```

Expected: clean compile.

- [ ] **Commit:**

```bash
git add src/state.rs
git commit -m "feat: add add_relay, remove_relay, relays to NostrSyncState"
```

---

## Task 9: Payload size check and NIP-44 encrypt/decrypt helpers (TDD)

**Files:**
- Modify: `src/state.rs`

The 64KB limit is checked against the JSON byte length before any relay interaction.

- [ ] **Add stub helpers and tests to `src/state.rs`:**

```rust
const PAYLOAD_LIMIT: usize = 64 * 1024; // 64KB

async fn encrypt_payload(
    signer: &Arc<dyn NostrSigner + Send + Sync>,
    payload: &serde_json::Value,
) -> Result<String> {
    todo!("implement encrypt_payload")
}

async fn decrypt_payload(
    signer: &Arc<dyn NostrSigner + Send + Sync>,
    ciphertext: &str,
) -> Result<serde_json::Value> {
    todo!("implement decrypt_payload")
}

fn check_payload_size(json: &str) -> Result<()> {
    todo!("implement check_payload_size")
}
```

Add these tests to the `#[cfg(test)]` block:

```rust
    #[tokio::test]
    async fn payload_encrypt_decrypt_roundtrip() {
        let keys = nostr_sdk::Keys::generate();
        let signer: Arc<dyn NostrSigner + Send + Sync> = Arc::new(keys);
        let original = serde_json::json!({ "theme": "dark", "font_size": 14 });
        let encrypted = encrypt_payload(&signer, &original).await.unwrap();
        let decrypted = decrypt_payload(&signer, &encrypted).await.unwrap();
        assert_eq!(original, decrypted);
    }

    #[test]
    fn payload_at_limit_is_accepted() {
        let json = "x".repeat(PAYLOAD_LIMIT);
        assert!(check_payload_size(&json).is_ok());
    }

    #[test]
    fn payload_over_limit_is_rejected() {
        let json = "x".repeat(PAYLOAD_LIMIT + 1);
        let result = check_payload_size(&json);
        assert!(matches!(result, Err(Error::PayloadTooLarge { .. })));
    }
```

- [ ] **Run tests and confirm they panic (todo! fires):**

```bash
cargo test payload_encrypt_decrypt_roundtrip
cargo test payload_at_limit
cargo test payload_over_limit
```

Expected: all `FAILED` with `not yet implemented`.

- [ ] **Implement the three functions:**

```rust
const PAYLOAD_LIMIT: usize = 64 * 1024;

fn check_payload_size(json: &str) -> Result<()> {
    let size = json.len();
    if size > PAYLOAD_LIMIT {
        return Err(Error::PayloadTooLarge { size, limit: PAYLOAD_LIMIT });
    }
    Ok(())
}

async fn encrypt_payload(
    signer: &Arc<dyn NostrSigner + Send + Sync>,
    payload: &serde_json::Value,
) -> Result<String> {
    let pubkey = signer
        .public_key()
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    let json = serde_json::to_string(payload)
        .map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    check_payload_size(&json)?;
    signer
        .nip44_encrypt(&pubkey, &json)
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))
}

async fn decrypt_payload(
    signer: &Arc<dyn NostrSigner + Send + Sync>,
    ciphertext: &str,
) -> Result<serde_json::Value> {
    let pubkey = signer
        .public_key()
        .await
        .map_err(|e| Error::DecryptionFailed(e.to_string()))?;
    let json = signer
        .nip44_decrypt(&pubkey, ciphertext)
        .await
        .map_err(|e| Error::DecryptionFailed(e.to_string()))?;
    serde_json::from_str(&json).map_err(|e| Error::DecryptionFailed(e.to_string()))
}
```

- [ ] **Run the tests and confirm they pass:**

```bash
cargo test payload
```

Expected: all three tests pass.

- [ ] **Commit:**

```bash
git add src/state.rs
git commit -m "feat: add payload size check and NIP-44 encrypt/decrypt with tests"
```

---

## Task 10: publish method

**Files:**
- Modify: `src/state.rs`

- [ ] **Add the necessary imports** at the top of `src/state.rs` (merge with existing imports):

```rust
use nostr_sdk::{EventBuilder, Filter, Kind, Tag};
use std::time::Duration;
```

- [ ] **Add `publish` to the `impl NostrSyncState` block:**

```rust
    pub async fn publish(&self, category: &str, payload: &serde_json::Value) -> Result<()> {
        let guard = self.signer.read().await;
        let signer = guard.as_ref().ok_or(Error::SignerNotSet)?;

        let ciphertext = encrypt_payload(signer, payload).await?;
        let dtag = build_dtag(&self.namespace, category);
        let kind = Kind::from(30078u16);

        let pubkey = signer
            .public_key()
            .await
            .map_err(|e| Error::EncryptionFailed(e.to_string()))?;

        let unsigned = EventBuilder::new(kind, ciphertext)
            .tag(Tag::identifier(dtag))
            .build(pubkey);

        let event = signer
            .sign_event(unsigned)
            .await
            .map_err(|e| Error::EncryptionFailed(e.to_string()))?;

        self.client.send_event(event).await?;
        Ok(())
    }
```

Note on API: `Tag::identifier(dtag)` creates a `#d` tag. `EventBuilder::build(pubkey)` creates an `UnsignedEvent`. `signer.sign_event(unsigned)` signs it. Verify these method names with `cargo doc --open` — they may differ slightly in your installed version.

- [ ] **Run `cargo check`:**

```bash
cargo check
```

Expected: clean compile.

- [ ] **Commit:**

```bash
git add src/state.rs
git commit -m "feat: add publish method to NostrSyncState"
```

---

## Task 11: timestamp comparison and fetch method (TDD)

**Files:**
- Modify: `src/state.rs`

- [ ] **Add timestamp helper stub and tests to `src/state.rs`:**

```rust
/// Returns true if `incoming` is strictly newer than `known`, or if there is no known timestamp.
fn is_newer(known: Option<&Timestamp>, incoming: &Timestamp) -> bool {
    todo!("implement is_newer")
}
```

Add these tests to the `#[cfg(test)]` block:

```rust
    #[test]
    fn timestamp_newer_wins() {
        let old = Timestamp::from(100u64);
        let new = Timestamp::from(200u64);
        assert!(is_newer(Some(&old), &new));
    }

    #[test]
    fn timestamp_equal_is_discarded() {
        let t = Timestamp::from(100u64);
        assert!(!is_newer(Some(&t), &t));
    }

    #[test]
    fn timestamp_older_is_discarded() {
        let old = Timestamp::from(100u64);
        let new = Timestamp::from(200u64);
        assert!(!is_newer(Some(&new), &old));
    }

    #[test]
    fn timestamp_no_known_is_always_newer() {
        let t = Timestamp::from(100u64);
        assert!(is_newer(None, &t));
    }
```

- [ ] **Run tests to confirm they panic:**

```bash
cargo test timestamp_
```

Expected: all `FAILED` with `not yet implemented`.

- [ ] **Implement `is_newer`:**

```rust
fn is_newer(known: Option<&Timestamp>, incoming: &Timestamp) -> bool {
    match known {
        None => true,
        Some(k) => incoming > k,
    }
}
```

- [ ] **Run tests and confirm they pass:**

```bash
cargo test timestamp_
```

Expected: all four tests pass.

- [ ] **Add `fetch` to the `impl NostrSyncState` block:**

```rust
    pub async fn fetch(&self, category: &str) -> Result<Option<crate::FetchResult>> {
        let guard = self.signer.read().await;
        let signer = guard.as_ref().ok_or(Error::SignerNotSet)?;

        let pubkey = signer
            .public_key()
            .await
            .map_err(|e| Error::DecryptionFailed(e.to_string()))?;

        let dtag = build_dtag(&self.namespace, category);
        let filter = Filter::new()
            .kind(Kind::from(30078u16))
            .author(pubkey)
            .identifier(dtag.clone());

        let timeout = Duration::from_secs(10);
        let mut events = self.client.fetch_events(vec![filter], timeout).await?;

        // Sort descending by created_at; take the newest.
        events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let event = match events.into_iter().next() {
            Some(e) => e,
            None => return Ok(None),
        };

        // Only surface the event if it's newer than what we've already seen.
        let mut timestamps = self.known_timestamps.write().await;
        let known = timestamps.get(category);
        if !is_newer(known, &event.created_at) {
            return Ok(None);
        }

        let payload = decrypt_payload(signer, &event.content).await?;

        // Extract device_id from event tags (tag name "device_id").
        // If absent (e.g. published by another client), use pubkey as fallback.
        // Extract device_id from custom tag. nostr-sdk tag API varies by version;
        // if this doesn't compile, replace with a simple linear scan over event.tags
        // looking for a tag whose first element is "device_id".
        let device_id = event
            .tags
            .iter()
            .find_map(|t| {
                let vec = t.to_vec();
                if vec.first().map(|s| s.as_str()) == Some("device_id") {
                    vec.get(1).cloned()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| pubkey.to_hex());

        timestamps.insert(category.to_string(), event.created_at);

        let updated_at = chrono::DateTime::from_timestamp_opt(event.created_at.as_u64() as i64, 0)
            .unwrap_or_else(chrono::Utc::now);

        Ok(Some(crate::FetchResult {
            payload,
            updated_at,
            device_id,
        }))
    }
```

Note: `client.fetch_events` may be named `get_events_of` in your installed version — check `cargo doc --open`. The `Tag::kind()` and `Tag::content()` APIs also vary — use `cargo doc` to verify.

- [ ] **Update `publish` to also attach the device_id tag** (modify the existing `publish` method to add a tag before `.build(pubkey)`):

```rust
        // Tag::custom takes a TagKind and a list of string values.
        // If TagKind::custom doesn't exist in your version, use Tag::parse(vec!["device_id", &self.device_id])
        let unsigned = EventBuilder::new(kind, ciphertext)
            .tag(Tag::identifier(dtag))
            .tag(Tag::parse(vec!["device_id", &self.device_id])
                .unwrap_or_else(|_| Tag::identifier("unknown")))
            .build(pubkey);
```

- [ ] **Run all tests:**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Commit:**

```bash
git add src/state.rs
git commit -m "feat: add is_newer helper and fetch method with timestamp tests"
```

---

## Task 12: Update desktop.rs to wrap NostrSyncState

**Files:**
- Modify: `src/desktop.rs`

- [ ] **Replace `src/desktop.rs`:**

```rust
use std::sync::Arc;

use tauri::{AppHandle, Runtime};

use crate::{FetchResult, NostrSyncState, RelayInfo, Result, SyncStatus};

pub fn init<R: Runtime>(app: &AppHandle<R>) -> crate::Result<TauriPluginNostrSync<R>> {
    let state = Arc::new(NostrSyncState::new("default")?);
    Ok(TauriPluginNostrSync {
        _app: app.clone(),
        pub_state: state,
    })
}

pub struct TauriPluginNostrSync<R: Runtime> {
    _app: AppHandle<R>,
    pub pub_state: Arc<NostrSyncState>,
}

impl<R: Runtime> TauriPluginNostrSync<R> {
    pub async fn set_signer(
        &self,
        signer: impl nostr_sdk::NostrSigner + Send + Sync + 'static,
    ) -> Result<()> {
        self.pub_state.set_signer(signer).await
    }

    pub async fn clear_signer(&self) {
        self.pub_state.clear_signer().await
    }

    pub async fn status(&self) -> SyncStatus {
        self.pub_state.status().await
    }

    pub async fn pubkey(&self) -> Option<nostr_sdk::PublicKey> {
        self.pub_state.pubkey().await
    }

    pub async fn add_relay(&self, url: &str) -> Result<()> {
        self.pub_state.add_relay(url).await
    }

    pub async fn remove_relay(&self, url: &str) -> Result<()> {
        self.pub_state.remove_relay(url).await
    }

    pub async fn relays(&self) -> Vec<RelayInfo> {
        self.pub_state.relays().await
    }

    pub async fn publish(&self, category: &str, payload: &serde_json::Value) -> Result<()> {
        self.pub_state.publish(category, payload).await
    }

    pub async fn fetch(&self, category: &str) -> Result<Option<FetchResult>> {
        self.pub_state.fetch(category).await
    }
}
```

Note: The `init` function signature changed — it no longer takes a `PluginApi` argument. Update the call in `lib.rs` accordingly (it already matches the stub we wrote in Task 1). Also, the namespace is hardcoded to `"default"` for now — Phase 2 introduces the `Builder` pattern that allows the host app to configure it.

- [ ] **Run `cargo check`:**

```bash
cargo check
```

Fix any compile errors. Common ones:
- `init` signature mismatch between `lib.rs` and `desktop.rs` — align them.
- Missing `use` items — add as needed.

- [ ] **Commit:**

```bash
git add src/desktop.rs src/lib.rs
git commit -m "feat: update desktop.rs to delegate to NostrSyncState"
```

---

## Task 13: Final verification

- [ ] **Run the full test suite:**

```bash
cargo test
```

Expected output (all pass):
```
test outbox::tests::backoff_is_exponential_up_to_cap ... ok
test state::tests::dtag_format_includes_namespace_category_and_version ... ok
test state::tests::dtag_works_with_various_inputs ... ok
test state::tests::namespace_rejects_empty_string ... ok
test state::tests::namespace_rejects_slash ... ok
test state::tests::namespace_accepts_valid_identifier ... ok
test state::tests::sync_status_not_ready_without_signer ... ok
test state::tests::payload_at_limit_is_accepted ... ok
test state::tests::payload_over_limit_is_rejected ... ok
test state::tests::payload_encrypt_decrypt_roundtrip ... ok
test state::tests::timestamp_newer_wins ... ok
test state::tests::timestamp_equal_is_discarded ... ok
test state::tests::timestamp_older_is_discarded ... ok
test state::tests::timestamp_no_known_is_always_newer ... ok
```

- [ ] **Run clippy:**

```bash
cargo clippy -- -D warnings
```

Fix any warnings.

- [ ] **Commit any cleanup:**

```bash
git add -p
git commit -m "chore: phase 1 cleanup from clippy"
```

---

## Phase 1 Complete

After all 13 tasks, Phase 1 delivers:
- A renamed, compilable `tauri-plugin-nostr-sync` crate with full Nostr dependencies
- `NostrSyncState` with relay management, runtime signer injection, NIP-44 encrypt/decrypt, publish, and fetch
- 14 passing unit tests covering all pure logic
- No Tauri IPC wiring yet (that is Phase 2)

**Phase 2 plan** covers: Builder pattern, all 8 Tauri commands, TypeScript bindings, permissions, and the full example app with Bootstrap UI. Write the Phase 2 plan using `superpowers:writing-plans` once Phase 1 is merged and green.
