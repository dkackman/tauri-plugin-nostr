# Configurable Payload Size Limit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the max payload size configurable via `PluginBuilder`, defaulting to 64KB with a hard cap of 400KB, surfacing over-cap values as a catchable `Error::InvalidPayloadLimit` through Tauri's setup closure.

**Architecture:** The limit travels from `PluginBuilder` → `desktop::init()` / `mobile::init()` → `NostrSyncState::new()`, where it is stored as an instance field. `check_payload_size` becomes a free function taking an explicit `limit` parameter, called from `encrypt_payload` which also receives the limit. No IPC commands or TypeScript bindings change.

**Tech Stack:** Rust, Tauri 2.x plugin system, `thiserror`, `nostr-sdk`

---

## File Map

| File | Change |
| --- | --- |
| `src/error.rs` | Add `InvalidPayloadLimit` variant |
| `src/state.rs` | Add `DEFAULT_PAYLOAD_LIMIT` / `MAX_PAYLOAD_LIMIT` consts; add `max_payload_size` field; update `new()` signature + validation; update `check_payload_size` and `encrypt_payload` signatures; update tests |
| `src/builder.rs` | Add `max_payload_size` field and `.max_payload_size()` setter; thread value into setup closure; add builder tests |
| `src/desktop.rs` | Add `max_payload_size: usize` param to `init()` |
| `src/mobile.rs` | Add `max_payload_size: usize` param to `init()` |
| `README.md` | Add `.max_payload_size()` to builder example; add payload limit note |
| `specs/tauri-plugin-nostr.md` | Update builder snippet; add constraint table rows |

---

## Task 1: Add `InvalidPayloadLimit` error variant

**Files:**
- Modify: `src/error.rs`

- [ ] **Step 1: Add the variant**

In `src/error.rs`, add after the `NoRelaysAccepted` variant:

```rust
#[error("payload limit {requested} bytes exceeds the {max} byte maximum")]
InvalidPayloadLimit { requested: usize, max: usize },
```

The full enum after the change:

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Nostr(#[from] nostr_sdk::client::Error),

    #[error("signer not set — call set_signer before publishing or fetching")]
    SignerNotSet,

    #[error("payload too large: {size} bytes exceeds {limit} byte limit")]
    PayloadTooLarge { size: usize, limit: usize },

    #[error("encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("signing failed: {0}")]
    SigningFailed(String),

    #[error("decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("invalid namespace '{0}': must be non-empty and contain no '/' characters")]
    InvalidNamespace(String),

    #[error("invalid category '{0}': must be non-empty and contain no '/' characters")]
    InvalidCategory(String),

    #[error("no relays accepted the event")]
    NoRelaysAccepted,

    #[error("payload limit {requested} bytes exceeds the {max} byte maximum")]
    InvalidPayloadLimit { requested: usize, max: usize },

    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/error.rs
git commit -m "feat: add InvalidPayloadLimit error variant"
```

---

## Task 2: Update `NostrSyncState` with constants, field, and validation

**Files:**
- Modify: `src/state.rs`

- [ ] **Step 1: Write the new failing tests**

Inside the `#[cfg(test)] mod tests` block in `src/state.rs`, add the following tests. They will fail to compile until the implementation is in place.

```rust
#[test]
fn new_rejects_payload_limit_over_max() {
    assert!(matches!(
        NostrSyncState::new("testapp", "test-device", MAX_PAYLOAD_LIMIT + 1),
        Err(Error::InvalidPayloadLimit { .. })
    ));
}

#[test]
fn new_accepts_payload_limit_at_max() {
    assert!(NostrSyncState::new("testapp", "test-device", MAX_PAYLOAD_LIMIT).is_ok());
}

#[test]
fn custom_limit_is_enforced() {
    let limit = 100;
    assert!(check_payload_size(&"x".repeat(limit), limit).is_ok());
    assert!(matches!(
        check_payload_size(&"x".repeat(limit + 1), limit),
        Err(Error::PayloadTooLarge { .. })
    ));
}
```

- [ ] **Step 2: Run tests to confirm they fail to compile**

```bash
cargo test 2>&1 | head -30
```

Expected: compile errors referencing `MAX_PAYLOAD_LIMIT`, the new `new()` signature, and `check_payload_size` arity.

- [ ] **Step 3: Replace the module-level constant with two public constants**

Replace this line at `src/state.rs:265`:

```rust
const PAYLOAD_LIMIT: usize = 64 * 1024; // 64KB
```

With:

```rust
pub const DEFAULT_PAYLOAD_LIMIT: usize = 64 * 1024;  // 64KB default
pub const MAX_PAYLOAD_LIMIT: usize = 400 * 1024;     // 400KB hard cap
```

- [ ] **Step 4: Add `max_payload_size` field to `NostrSyncState`**

Replace the struct definition:

```rust
pub struct NostrSyncState {
    pub(crate) namespace: String,
    pub(crate) device_id: String,
    pub(crate) client: Client,
    last_seen: tokio::sync::RwLock<std::collections::HashMap<String, Timestamp>>,
}
```

With:

```rust
pub struct NostrSyncState {
    pub(crate) namespace: String,
    pub(crate) device_id: String,
    pub(crate) client: Client,
    max_payload_size: usize,
    last_seen: tokio::sync::RwLock<std::collections::HashMap<String, Timestamp>>,
}
```

- [ ] **Step 5: Update `new()` to accept and validate `max_payload_size`**

Replace the `new` method:

```rust
pub fn new(namespace: &str, device_id: &str) -> Result<Self> {
    validate_namespace(namespace)?;
    let opts = ClientOptions::default().autoconnect(true);
    let client = Client::builder().opts(opts).build();
    Ok(Self {
        namespace: namespace.to_string(),
        device_id: device_id.to_string(),
        client,
        last_seen: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    })
}
```

With:

```rust
pub fn new(namespace: &str, device_id: &str, max_payload_size: usize) -> Result<Self> {
    validate_namespace(namespace)?;
    if max_payload_size > MAX_PAYLOAD_LIMIT {
        return Err(Error::InvalidPayloadLimit {
            requested: max_payload_size,
            max: MAX_PAYLOAD_LIMIT,
        });
    }
    let opts = ClientOptions::default().autoconnect(true);
    let client = Client::builder().opts(opts).build();
    Ok(Self {
        namespace: namespace.to_string(),
        device_id: device_id.to_string(),
        client,
        max_payload_size,
        last_seen: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    })
}
```

- [ ] **Step 6: Update `check_payload_size` to accept an explicit limit**

Replace:

```rust
fn check_payload_size(json: &str) -> Result<()> {
    let size = json.len();
    if size > PAYLOAD_LIMIT {
        return Err(Error::PayloadTooLarge {
            size,
            limit: PAYLOAD_LIMIT,
        });
    }
    Ok(())
}
```

With:

```rust
fn check_payload_size(json: &str, limit: usize) -> Result<()> {
    let size = json.len();
    if size > limit {
        return Err(Error::PayloadTooLarge { size, limit });
    }
    Ok(())
}
```

- [ ] **Step 7: Update `encrypt_payload` to accept and pass through the limit**

Replace:

```rust
async fn encrypt_payload(
    signer: &Arc<dyn NostrSigner>,
    payload: &serde_json::Value,
) -> Result<String> {
    let pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    let json =
        serde_json::to_string(payload).map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    check_payload_size(&json)?;
    signer
        .nip44_encrypt(&pubkey, &json)
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))
}
```

With:

```rust
async fn encrypt_payload(
    signer: &Arc<dyn NostrSigner>,
    payload: &serde_json::Value,
    limit: usize,
) -> Result<String> {
    let pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    let json =
        serde_json::to_string(payload).map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    check_payload_size(&json, limit)?;
    signer
        .nip44_encrypt(&pubkey, &json)
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))
}
```

- [ ] **Step 8: Update the `encrypt_payload` call in `publish()`**

In the `publish` method, find:

```rust
let ciphertext = encrypt_payload(&signer, payload).await?;
```

Replace with:

```rust
let ciphertext = encrypt_payload(&signer, payload, self.max_payload_size).await?;
```

- [ ] **Step 9: Update all existing `NostrSyncState::new()` call sites in the test module**

Every call to `NostrSyncState::new("testapp", "test-device")` in the test block must gain a third argument. Replace all occurrences of:

```rust
NostrSyncState::new("testapp", "test-device")
```

With:

```rust
NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT)
```

Also update the two calls in `new_rejects_invalid_namespace`:

```rust
NostrSyncState::new("", "test-device")
```
→
```rust
NostrSyncState::new("", "test-device", DEFAULT_PAYLOAD_LIMIT)
```

```rust
NostrSyncState::new("a/b", "test-device")
```
→
```rust
NostrSyncState::new("a/b", "test-device", DEFAULT_PAYLOAD_LIMIT)
```

- [ ] **Step 10: Update the two existing payload size tests**

Replace:

```rust
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

With:

```rust
#[test]
fn payload_at_limit_is_accepted() {
    let json = "x".repeat(DEFAULT_PAYLOAD_LIMIT);
    assert!(check_payload_size(&json, DEFAULT_PAYLOAD_LIMIT).is_ok());
}

#[test]
fn payload_over_limit_is_rejected() {
    let json = "x".repeat(DEFAULT_PAYLOAD_LIMIT + 1);
    assert!(matches!(
        check_payload_size(&json, DEFAULT_PAYLOAD_LIMIT),
        Err(Error::PayloadTooLarge { .. })
    ));
}
```

- [ ] **Step 11: Run all tests and confirm they pass**

```bash
cargo test
```

Expected: all tests pass, no warnings about unused `PAYLOAD_LIMIT`.

- [ ] **Step 12: Commit**

```bash
git add src/state.rs
git commit -m "feat: make payload size limit configurable on NostrSyncState"
```

---

## Task 3: Thread `max_payload_size` through `PluginBuilder`, `desktop::init`, and `mobile::init`

**Files:**
- Modify: `src/builder.rs`
- Modify: `src/desktop.rs`
- Modify: `src/mobile.rs`

- [ ] **Step 1: Write failing builder tests**

Add to the `#[cfg(test)] mod tests` block in `src/builder.rs`:

```rust
#[test]
fn max_payload_size_defaults_to_64kb() {
    let b = PluginBuilder::new();
    assert_eq!(b.max_payload_size, crate::state::DEFAULT_PAYLOAD_LIMIT);
}

#[test]
fn max_payload_size_setter_stores_value() {
    let b = PluginBuilder::new().max_payload_size(128 * 1024);
    assert_eq!(b.max_payload_size, 128 * 1024);
}
```

- [ ] **Step 2: Run tests to confirm they fail to compile**

```bash
cargo test 2>&1 | head -20
```

Expected: compile errors — `max_payload_size` field does not exist on `PluginBuilder`.

- [ ] **Step 3: Add `max_payload_size` field and setter to `PluginBuilder`**

Replace the struct definition:

```rust
pub struct PluginBuilder {
    pub(crate) relays: Vec<String>,
    pub(crate) namespace: String,
    pub(crate) device_id: String,
}
```

With:

```rust
pub struct PluginBuilder {
    pub(crate) relays: Vec<String>,
    pub(crate) namespace: String,
    pub(crate) device_id: String,
    pub(crate) max_payload_size: usize,
}
```

Update `PluginBuilder::new()`:

```rust
pub fn new() -> Self {
    Self {
        relays: Vec::new(),
        namespace: "default".to_string(),
        device_id: uuid::Uuid::new_v4().to_string(),
        max_payload_size: crate::state::DEFAULT_PAYLOAD_LIMIT,
    }
}
```

Add the setter after the `device_id` setter:

```rust
/// Override the maximum payload size in bytes. Defaults to 64KB. Must not exceed 400KB.
///
/// Payloads exceeding this limit return `Error::PayloadTooLarge` from `publish`.
/// Values above 400KB surface as `Error::InvalidPayloadLimit` at plugin startup.
pub fn max_payload_size(mut self, bytes: usize) -> Self {
    self.max_payload_size = bytes;
    self
}
```

- [ ] **Step 4: Thread `max_payload_size` through `build()`**

In `PluginBuilder::build()`, capture the new field alongside the existing ones and pass it to `init`:

```rust
pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
    // Panic on invalid namespace — consistent with Tauri builder conventions.
    crate::state::validate_namespace(&self.namespace)
        .unwrap_or_else(|e| panic!("invalid app_namespace: {e}"));

    let relays = self.relays;
    let namespace = self.namespace;
    let device_id = self.device_id;
    let max_payload_size = self.max_payload_size;

    tauri::plugin::Builder::<R>::new("nostr-sync")
        .invoke_handler(tauri::generate_handler![
            crate::commands::publish,
            crate::commands::fetch,
            crate::commands::sync_all,
            crate::commands::add_relay,
            crate::commands::remove_relay,
            crate::commands::get_relays,
            crate::commands::get_pubkey,
            crate::commands::get_status,
            crate::commands::poll,
        ])
        .setup(move |app, api| {
            #[cfg(mobile)]
            {
                let plugin = crate::mobile::init(app, api, relays, &namespace, &device_id, max_payload_size)?;
                app.manage(plugin);
            }
            #[cfg(desktop)]
            {
                let _ = &api;
                let plugin = crate::desktop::init(app, relays, &namespace, &device_id, max_payload_size)?;
                app.manage(plugin);
            }
            Ok(())
        })
        .build()
}
```

- [ ] **Step 5: Update `desktop::init()` to accept and pass `max_payload_size`**

Replace the full `init` function signature and body in `src/desktop.rs`:

```rust
pub fn init<R: Runtime>(
    app: &AppHandle<R>,
    relays: Vec<String>,
    namespace: &str,
    device_id: &str,
    max_payload_size: usize,
) -> crate::Result<TauriPluginNostrSync<R>> {
    let state = Arc::new(NostrSyncState::new(namespace, device_id, max_payload_size)?);
    let plugin = TauriPluginNostrSync {
        app: app.clone(),
        pub_state: state,
    };
    // Relay connections are kicked off async; errors surface later via status().
    let state_clone = plugin.pub_state.clone();
    tauri::async_runtime::spawn(async move {
        for url in relays {
            let _ = state_clone.add_relay(&url).await;
        }
    });
    Ok(plugin)
}
```

- [ ] **Step 6: Update `mobile::init()` to accept and pass `max_payload_size`**

Replace the `init` function signature in `src/mobile.rs`:

```rust
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    api: PluginApi<R, C>,
    relays: Vec<String>,
    namespace: &str,
    device_id: &str,
    max_payload_size: usize,
) -> crate::Result<TauriPluginNostrSync<R>> {
    // Register the native no-op plugin to satisfy Tauri's mobile lifecycle.
    // The handle is intentionally dropped — all logic runs in Rust.
    #[cfg(target_os = "android")]
    let _ = api.register_android_plugin("app.tauri.plugin.nostr", "NostrSyncPlugin");
    #[cfg(target_os = "ios")]
    let _ = api.register_ios_plugin(init_plugin_tauri_plugin_nostr_sync);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let _ = &api;

    let state = Arc::new(NostrSyncState::new(namespace, device_id, max_payload_size)?);
    let plugin = TauriPluginNostrSync {
        app: app.clone(),
        pub_state: state,
    };
    let state_clone = plugin.pub_state.clone();
    tauri::async_runtime::spawn(async move {
        for url in relays {
            let _ = state_clone.add_relay(&url).await;
        }
    });
    Ok(plugin)
}
```

- [ ] **Step 7: Run all tests and confirm they pass**

```bash
cargo test
```

Expected: all tests pass including the two new builder tests.

- [ ] **Step 8: Commit**

```bash
git add src/builder.rs src/desktop.rs src/mobile.rs
git commit -m "feat: thread max_payload_size through PluginBuilder and platform init"
```

---

## Task 4: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `specs/tauri-plugin-nostr.md`

- [ ] **Step 1: Update the builder example in `README.md`**

Find the setup code block (around line 42):

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

Replace with:

```rust
tauri::Builder::default()
    .plugin(
        tauri_plugin_nostr_sync::Builder::new()
            .relays(vec![
                "wss://relay.damus.io",
                "wss://relay.nostr.band",
                "wss://nos.lol",
            ])
            .app_namespace("myapp")      // prefixes all d-tags
            .max_payload_size(128 * 1024) // optional: default 64KB, cap 400KB
            .build()
    )
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

- [ ] **Step 2: Add a payload limit note to `README.md`**

After the convenience alias line (`tauri_plugin_nostr_sync::init()` is a convenience alias…), add:

```markdown
### Payload size limit

The default maximum payload size is **64KB**. Use `.max_payload_size(bytes)` on the builder to increase it up to **400KB**. Values above 400KB are rejected at plugin startup via Tauri's setup error path. `publish` returns `Error::PayloadTooLarge` when the serialized payload exceeds the configured limit.
```

- [ ] **Step 3: Update the builder snippet in `specs/tauri-plugin-nostr.md`**

Find the Plugin Registration code block (around line 45):

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

Replace with:

```rust
tauri::Builder::default()
    .plugin(
        tauri_plugin_nostr_sync::Builder::new()
            .relays(vec![
                "wss://relay.damus.io",
                "wss://relay.nostr.band",
                "wss://nos.lol",
            ])
            .app_namespace("sage")        // prefixes all d-tags
            .max_payload_size(128 * 1024) // optional: default 64KB, cap 400KB
            .build()
    )
```

- [ ] **Step 4: Add payload limit rows to the design constraints table in `specs/tauri-plugin-nostr.md`**

Find the Design Constraints section. In the constraints list, add after the existing payload size constraint (or create a new payload group):

```markdown
- **Payload size limit configurable** — `PluginBuilder::max_payload_size(bytes)` sets the per-publish limit; defaults to 64KB, hard cap 400KB. Values over the cap return `Error::InvalidPayloadLimit` through the Tauri plugin setup path.
```

- [ ] **Step 5: Verify everything still compiles and tests pass**

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add README.md specs/tauri-plugin-nostr.md
git commit -m "docs: document configurable payload size limit"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Covered by |
| --- | --- |
| `max_payload_size` field on `PluginBuilder` | Task 3, Step 3 |
| `.max_payload_size(bytes) -> Self` setter | Task 3, Step 3 |
| Default of `DEFAULT_PAYLOAD_LIMIT` (64KB) | Task 2, Step 3; Task 3, Step 3 |
| Hard cap `MAX_PAYLOAD_LIMIT` (400KB) | Task 2, Step 3 |
| `Error::InvalidPayloadLimit` on over-cap | Task 1; Task 2, Step 5 |
| Error surfaces via Tauri setup closure (catchable) | Task 3, Step 4 |
| `NostrSyncState` stores and uses instance limit | Task 2, Steps 4–8 |
| `check_payload_size` and `encrypt_payload` updated | Task 2, Steps 6–8 |
| All existing `new()` call sites updated | Task 2, Step 9 |
| Existing payload tests updated | Task 2, Step 10 |
| New tests: over-cap, at-cap, custom limit, builder defaults | Task 2, Steps 1+10; Task 3, Step 1 |
| `desktop::init()` threaded | Task 3, Step 5 |
| `mobile::init()` threaded | Task 3, Step 6 |
| `README.md` updated | Task 4, Steps 1–2 |
| `specs/tauri-plugin-nostr.md` updated | Task 4, Steps 3–4 |

**Placeholder scan:** No TBDs or incomplete steps. All code blocks are complete.

**Type consistency:** `max_payload_size: usize` used consistently across all files. `DEFAULT_PAYLOAD_LIMIT` / `MAX_PAYLOAD_LIMIT` referenced by their exact names everywhere. `Error::InvalidPayloadLimit { requested, max }` field names match between definition (Task 1) and construction (Task 2, Step 5).
