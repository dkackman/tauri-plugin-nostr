# iOS & Android Implementation Design
**Date:** 2026-05-05
**Plugin:** tauri-plugin-nostr-sync

---

## Goal

Replace the current `mobile.rs` stub (which returns "not supported" errors for every command) with a full implementation that reuses the existing `NostrSyncState` Rust state machine on both iOS and Android. Feature parity with desktop: publish, fetch, sync_all, poll, relay management, signer lifecycle, status.

---

## Approach

Pure Rust on mobile. `nostr_sdk` is pure async Rust with no platform-specific dependencies — WebSocket relay connections, NIP-44 crypto, and NIP-33 event signing all cross-compile to `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios`, and `aarch64-linux-android` without modification.

The Swift and Kotlin native plugin files are kept as minimal no-ops to satisfy Tauri's build system (Xcode links against the `ios_plugin_binding!`-generated symbol; removing it breaks linking). They receive no meaningful calls.

---

## Files Changed

### `src/mobile.rs` — Full rewrite

**Before:** Holds `PluginHandle<R>`; all methods return `Err("not supported")`.

**After:** Mirrors `desktop.rs`:
- Struct holds `AppHandle<R>` + `Arc<NostrSyncState>`
- `init<R, C>(app, api, relays, namespace, device_id)` constructs `NostrSyncState`, spawns relay connections async
- iOS: calls `api.register_ios_plugin(init_plugin_tauri_plugin_nostr_sync)` and drops the handle (no-op Swift plugin)
- Android: calls `api.register_android_plugin(...)` and drops the handle (no-op Kotlin plugin)
- `ios_plugin_binding!` macro stays — required for Xcode symbol resolution
- All 11 methods delegate to `NostrSyncState`, same as desktop
- `poll` emits `nostr-sync://updated` events via `AppHandle`, same as desktop

### `src/builder.rs` — Mobile branch updated

```rust
#[cfg(mobile)]
{
    let plugin = crate::mobile::init(app, api, relays, &namespace, &device_id)?;
    app.manage(plugin);
}
```

Currently the mobile branch only passes `(app, api)`. Add `relays`, `namespace`, `device_id` to match the new `mobile::init` signature.

### `ios/Sources/ExamplePlugin.swift` → `ios/Sources/NostrSyncPlugin.swift`

- Rename file
- Rename class `ExamplePlugin` → `NostrSyncPlugin` with empty body
- Fix `@_cdecl` name: `"init_plugin_tauri_plugin_nostr"` → `"init_plugin_tauri_plugin_nostr_sync"` to match `ios_plugin_binding!`

### `android/src/main/java/ExamplePlugin.kt` → `android/src/main/java/NostrSyncPlugin.kt`

- Rename file and class
- Keep as minimal empty plugin

---

## What Does Not Change

- `src/desktop.rs` — untouched
- `src/state.rs` — untouched; `NostrSyncState` is platform-agnostic
- `src/commands.rs` — untouched
- `guest-js/`, `permissions/`, `build.rs` — untouched
- All existing tests — pass unchanged

---

## Testing

No new tests required. The state machine logic is fully covered by existing tests in `src/state.rs` and `tests/`. After implementation, verify by running `cargo check` for each mobile target:

```bash
cargo check --target aarch64-apple-ios-sim
cargo check --target aarch64-apple-ios
cargo check --target x86_64-apple-ios
cargo check --target aarch64-linux-android
```

And run the full desktop test suite to confirm nothing regressed:

```bash
cargo test
```

---

## Constraints & Risks

- **`ios_plugin_binding!` must stay** — Tauri's Xcode project calls the generated C symbol at launch. Removing it causes a link error.
- **`@_cdecl` name mismatch** — The template shipped with `"init_plugin_tauri_plugin_nostr"` but `ios_plugin_binding!` uses `init_plugin_tauri_plugin_nostr_sync`. The Swift file must be corrected or Tauri's iOS runtime cannot find the Swift plugin entry point.
- **Android registration** — `api.register_android_plugin` is called and the handle dropped. The Kotlin class remains a stub; if Android IPC commands are ever needed natively in a future phase, the handle would be retained then.
