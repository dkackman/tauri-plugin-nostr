# iOS & Android Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `mobile.rs` "not supported" stubs with a full implementation that delegates to the existing `NostrSyncState` Rust state machine, giving iOS and Android feature parity with desktop.

**Architecture:** `mobile.rs` is rewritten to mirror `desktop.rs` — it holds `AppHandle<R>` + `Arc<NostrSyncState>` and delegates all 11 methods to `NostrSyncState`. The Swift and Kotlin native plugin files become minimal no-ops kept only to satisfy Tauri's build system. No new logic is introduced; the state machine already handles all platform-agnostic Nostr relay work.

**Tech Stack:** Rust (nostr-sdk, tauri 2.x), Swift (no-op Tauri plugin), Kotlin (no-op Tauri plugin)

---

## File Map

| File | Action | Notes |
| --- | --- | --- |
| `src/mobile.rs` | Rewrite | `Arc<NostrSyncState>` + `AppHandle<R>`; keep `ios_plugin_binding!` |
| `src/builder.rs` | Modify | Mobile branch passes `relays`, `namespace`, `device_id` |
| `ios/Sources/ExamplePlugin.swift` | Replace with `NostrSyncPlugin.swift` | Fix `@_cdecl` name; empty plugin body |
| `android/src/main/java/ExamplePlugin.kt` | Replace with `NostrSyncPlugin.kt` | Minimal no-op plugin |
| `android/src/main/java/Example.kt` | Delete | Only referenced by removed ExamplePlugin |

---

## Task 1: Rewrite `src/mobile.rs` and update `src/builder.rs`

These two files must change together — `mobile::init`'s new signature is called from `builder.rs`, so both need to compile in the same pass.

**Files:**

- Modify: `src/mobile.rs`
- Modify: `src/builder.rs`

- [ ] **Step 1: Replace `src/mobile.rs` with the full implementation**

Replace the entire file contents with:

```rust
use std::sync::Arc;

use serde::de::DeserializeOwned;
use tauri::{
    plugin::PluginApi,
    AppHandle, Emitter, Runtime,
};

use crate::{FetchResult, NostrSyncState, RelayInfo, Result, SyncStatus};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_tauri_plugin_nostr_sync);

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    api: PluginApi<R, C>,
    relays: Vec<String>,
    namespace: &str,
    device_id: &str,
) -> crate::Result<TauriPluginNostrSync<R>> {
    // Register the native no-op plugin to satisfy Tauri's mobile lifecycle.
    // The handle is intentionally dropped — all logic runs in Rust.
    #[cfg(target_os = "android")]
    let _ = api.register_android_plugin("tauri-plugin-nostr-sync", "NostrSyncPlugin");
    #[cfg(target_os = "ios")]
    let _ = api.register_ios_plugin(init_plugin_tauri_plugin_nostr_sync);
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    let _ = &api;

    let state = Arc::new(NostrSyncState::new(namespace, device_id)?);
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

pub struct TauriPluginNostrSync<R: Runtime> {
    app: AppHandle<R>,
    pub(crate) pub_state: Arc<NostrSyncState>,
}

impl<R: Runtime> TauriPluginNostrSync<R> {
    pub async fn set_signer(&self, signer: impl nostr_sdk::NostrSigner + 'static) -> Result<()> {
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

    pub async fn sync_all(&self, categories: &[String]) -> Result<Vec<FetchResult>> {
        self.pub_state.sync_all(categories).await
    }

    pub async fn wait_for_connection(&self, timeout: std::time::Duration) {
        self.pub_state.wait_for_connection(timeout).await
    }

    pub async fn poll(&self, categories: &[String]) -> Result<Vec<FetchResult>> {
        let updates = self.pub_state.poll(categories).await?;
        for update in &updates {
            let _ = self.app.emit("nostr-sync://updated", update);
        }
        Ok(updates)
    }
}
```

- [ ] **Step 2: Update the mobile branch in `src/builder.rs`**

Find this block in `builder.rs` (around line 61–65):

```rust
            #[cfg(mobile)]
            {
                let plugin = crate::mobile::init(app, api)?;
                app.manage(plugin);
            }
```

Replace with:

```rust
            #[cfg(mobile)]
            {
                let plugin = crate::mobile::init(app, api, relays, &namespace, &device_id)?;
                app.manage(plugin);
            }
```

- [ ] **Step 3: Verify the plugin compiles for iOS simulator**

```bash
cargo check --target aarch64-apple-ios-sim 2>&1 | grep -E "^error|^warning: unused|Finished"
```

Expected output (3 harmless unused-variable warnings from builder.rs, no errors):

```text
warning: unused variable: `relays`
warning: unused variable: `namespace`
warning: unused variable: `device_id`
Finished `dev` profile [unoptimized + debuginfo] target(s) in ...
```

These warnings appear because `relays`/`namespace`/`device_id` in the closure are only used inside the `#[cfg(mobile)]` block on non-desktop targets; on desktop they appear unused to the compiler. They are safe to ignore.

- [ ] **Step 4: Verify the desktop test suite still passes**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass, no compilation errors.

- [ ] **Step 5: Commit**

```bash
git add src/mobile.rs src/builder.rs
git commit -m "feat: implement iOS/Android using NostrSyncState Rust state machine"
```

---

## Task 2: Fix the iOS Swift plugin file

The template shipped with the wrong `@_cdecl` name (`init_plugin_tauri_plugin_nostr`) — it must match what `ios_plugin_binding!(init_plugin_tauri_plugin_nostr_sync)` expects, i.e. `init_plugin_tauri_plugin_nostr_sync`. This is a link-time requirement on iOS.

**Files:**

- Delete: `ios/Sources/ExamplePlugin.swift`
- Create: `ios/Sources/NostrSyncPlugin.swift`

- [ ] **Step 1: Delete the old file and create the corrected one**

Delete `ios/Sources/ExamplePlugin.swift` and create `ios/Sources/NostrSyncPlugin.swift` with:

```swift
import SwiftRs
import Tauri
import UIKit
import WebKit

class NostrSyncPlugin: Plugin {}

@_cdecl("init_plugin_tauri_plugin_nostr_sync")
func initPlugin() -> Plugin {
    NostrSyncPlugin()
}
```

- [ ] **Step 2: Commit**

```bash
git add ios/Sources/NostrSyncPlugin.swift ios/Sources/ExamplePlugin.swift
git commit -m "fix: rename iOS plugin class and fix @_cdecl name to match ios_plugin_binding"
```

---

## Task 3: Clean up Android plugin files

The Android plugin only needs to exist as an empty Tauri plugin class. `Example.kt` (referenced only by `ExamplePlugin.kt`) can be removed.

**Files:**

- Delete: `android/src/main/java/ExamplePlugin.kt`
- Delete: `android/src/main/java/Example.kt`
- Create: `android/src/main/java/NostrSyncPlugin.kt`

- [ ] **Step 1: Delete the old files and create the minimal plugin**

Delete both `android/src/main/java/ExamplePlugin.kt` and `android/src/main/java/Example.kt`.

Create `android/src/main/java/NostrSyncPlugin.kt` with:

```kotlin
package app.tauri.plugin.nostr

import android.app.Activity
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Plugin

@TauriPlugin
class NostrSyncPlugin(private val activity: Activity) : Plugin(activity)
```

- [ ] **Step 2: Commit**

```bash
git add android/src/main/java/NostrSyncPlugin.kt \
        android/src/main/java/ExamplePlugin.kt \
        android/src/main/java/Example.kt
git commit -m "fix: rename Android plugin class, remove unused Example stub"
```

---

## Task 4: Full mobile target verification

Run `cargo check` for every mobile Rust target to confirm the implementation compiles cleanly across all of them.

- [ ] **Step 1: Check all four mobile targets**

```bash
cargo check --target aarch64-apple-ios-sim 2>&1 | grep -E "^error|Finished"
cargo check --target aarch64-apple-ios     2>&1 | grep -E "^error|Finished"
cargo check --target x86_64-apple-ios      2>&1 | grep -E "^error|Finished"
cargo check --target aarch64-linux-android 2>&1 | grep -E "^error|Finished"
```

Expected for each:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in ...
```

No `error` lines. If any target prints an error, fix it before proceeding.

- [ ] **Step 2: Run the full desktop test suite one final time**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass.
