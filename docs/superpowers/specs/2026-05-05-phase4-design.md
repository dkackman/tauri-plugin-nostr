# Phase 4 Design — tauri-plugin-nostr-sync Example App

**Date:** 2026-05-05
**Scope:** Build out `examples/tauri-app/` into a working plugin demo that exercises all plugin commands including `poll`
**Out of scope:** Changes to the plugin itself, mobile support, automated UI tests

---

## Context

Phase 3 delivered the `poll` command with `nostr-sync://updated` Tauri event emission. The example app at `examples/tauri-app/` is currently a blank Tauri+Svelte template with a `greet` command placeholder. Phase 4 replaces it with a functional two-column Bootstrap app that demonstrates the full plugin surface.

---

## Layout

Two-column Bootstrap layout, dark-themed:

- **Left column (300px fixed):** IdentityCard, RelayCard, ActionsCard stacked vertically
- **Right column (flex-1):** SettingCard on top, ConsoleLog below filling remaining height

The app demonstrates one synced setting: `display-setting`, a JSON object with `{ name: string, color: string }`. This is sufficient to exercise publish, fetch, poll, and sync_all end-to-end.

---

## Section 1 — Component Architecture

Five Svelte 5 components, one per card:

| Component | File | Responsibility |
|---|---|---|
| `IdentityCard` | `src/lib/IdentityCard.svelte` | pubkey display, nsec paste input, Set Key / Generate / Clear buttons |
| `RelayCard` | `src/lib/RelayCard.svelte` | relay list with colored status dots, add URL input, remove button per relay |
| `ActionsCard` | `src/lib/ActionsCard.svelte` | all seven action buttons |
| `SettingCard` | `src/lib/SettingCard.svelte` | Display Name text input, Accent Color picker, Publish/Fetch buttons |
| `ConsoleLog` | `src/lib/ConsoleLog.svelte` | timestamped log entries with color coding, Clear button |

`App.svelte` owns:
- The two-column layout
- `pubkey: string` reactive state (empty until key is set; passed as prop to IdentityCard)
- `entries: LogEntry[]` reactive state (passed to ConsoleLog; appended via `log()` callback)
- `setting: { name: string, color: string }` reactive state (passed to and updated by SettingCard)
- The `nostr-sync://updated` Tauri event listener
- The `log(message, level)` callback passed as a prop to all five cards

No shared stores. The `log` callback and `pubkey` prop are the only cross-card wiring.

---

## Section 2 — Data Flow & Command Wiring

### Log Entry Format

```
HH:MM:SS [symbol] message
```

| Level | Symbol | Color |
|---|---|---|
| `success` | ✓ | green (`#22c55e`) |
| `event` | ● | blue (`#38bdf8`) |
| `error` | ✗ | red (`#ef4444`) |
| `info` | ℹ | gray (`#94a3b8`) |

### Button → Plugin Command Mapping

| Button | Invokes | On success |
|---|---|---|
| Set Key | `invoke("set_sync_key", { nsec })` | update `pubkey` prop, log ✓ |
| Generate | `invoke("generate_sync_key")` | populate nsec input, update `pubkey` prop, log ✓ |
| Clear | `unsetSigner()` | clear `pubkey`, log ℹ |
| Add Relay | `addRelay(url)` | clear input, log ✓ |
| Remove Relay (✕) | `removeRelay(url)` | log ✓ |
| Publish Setting | `publish("display-setting", setting)` | log ✓ |
| Fetch Setting | `fetch("display-setting")` | update `setting`, log ✓ |
| Poll for Updates | `poll(["display-setting"])` | log ✓ with update count |
| Sync All | `syncAll(["display-setting"])` | update `setting`, log ✓ |
| Get Status | `getStatus()` | log ℹ `ready=X relays=Y` |
| Get Relays | `getRelays()` | log ℹ relay list |
| Get Pubkey | `getPubkey()` | log ℹ pubkey or null |

### `nostr-sync://updated` Handler (in `App.svelte`)

```typescript
await listen("nostr-sync://updated", (event) => {
  const result = event.payload as FetchResult;
  log(`nostr-sync://updated ${JSON.stringify(result.payload)}`, "event");
  setting = result.payload as { name: string; color: string };
});
```

The app tracks one category (`display-setting`), so `setting` is updated unconditionally on any `nostr-sync://updated` event. `FetchResult` contains `payload`, `updatedAt`, and `deviceId` — no category field — so the unconditional update is the correct approach here.

### Relay Status Display

The relay list in RelayCard shows a colored dot per relay:
- Green (`#22c55e`) — connected
- Amber (`#f59e0b`) — connecting / disconnected
- Red (`#ef4444`) — failed

Relay status is fetched via `getRelays()`. The app re-fetches after add/remove operations. No automatic polling for relay status — the user can click "Get Relays" to refresh.

---

## Section 3 — Example App Rust Backend

Two new commands added to `examples/tauri-app/src-tauri/src/lib.rs`:

```rust
#[derive(serde::Serialize)]
struct KeyInfo {
    nsec: String,
    pubkey: String,
}

#[tauri::command]
async fn set_sync_key(app: AppHandle, nsec: String) -> Result<String, String> {
    let keys = Keys::parse(&nsec).map_err(|e| e.to_string())?;
    let pubkey = keys.public_key().to_hex();
    app.nostr_sync()
        .set_signer(NostrSigner::from(Arc::new(keys)))
        .await
        .map_err(|e| e.to_string())?;
    Ok(pubkey)
}

#[tauri::command]
async fn generate_sync_key(app: AppHandle) -> Result<KeyInfo, String> {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().map_err(|e| e.to_string())?;
    let pubkey = keys.public_key().to_hex();
    app.nostr_sync()
        .set_signer(NostrSigner::from(Arc::new(keys)))
        .await
        .map_err(|e| e.to_string())?;
    Ok(KeyInfo { nsec, pubkey })
}
```

Both commands handle key management as a host-app concern — raw key bytes never transit the IPC channel after being set. The `generate_sync_key` command returns the `nsec` once so the UI can display it for the user to copy (for cross-device use).

The example app's `Builder` registers both commands alongside the plugin init:

```rust
tauri::Builder::default()
    .plugin(
        tauri_plugin_nostr_sync::Builder::new()
            .relays(vec!["wss://relay.damus.io", "wss://nos.lol"])
            .build()
    )
    .invoke_handler(tauri::generate_handler![set_sync_key, generate_sync_key])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

The capabilities file (`src-tauri/capabilities/default.json`) already includes `"nostr-sync:default"` — no changes needed there.

---

## Section 4 — File Inventory

| File | Action |
|---|---|
| `examples/tauri-app/src-tauri/src/lib.rs` | Replace `greet` stub with `set_sync_key`, `generate_sync_key`, plugin init with relays |
| `examples/tauri-app/src/App.svelte` | Replace Tauri template with two-column Bootstrap layout + event listener |
| `examples/tauri-app/src/lib/IdentityCard.svelte` | New |
| `examples/tauri-app/src/lib/RelayCard.svelte` | New |
| `examples/tauri-app/src/lib/ActionsCard.svelte` | New |
| `examples/tauri-app/src/lib/SettingCard.svelte` | New |
| `examples/tauri-app/src/lib/ConsoleLog.svelte` | New |
| `examples/tauri-app/package.json` | Add `bootstrap` dependency |
| `examples/tauri-app/src/main.js` | Import `bootstrap/dist/css/bootstrap.min.css` |

No changes to the plugin crate, `guest-js/`, `dist-js/`, `permissions/`, or `build.rs`.

---

## Build Sequence

1. Add `bootstrap` to `examples/tauri-app/package.json` and import CSS
2. Write `src/lib/ConsoleLog.svelte` (no plugin calls — pure display)
3. Write `src/lib/IdentityCard.svelte`
4. Write `src/lib/RelayCard.svelte`
5. Write `src/lib/SettingCard.svelte`
6. Write `src/lib/ActionsCard.svelte`
7. Replace `src/App.svelte` with two-column layout wiring all five components
8. Replace `src-tauri/src/lib.rs` with `set_sync_key` + `generate_sync_key` + plugin init
9. Run `pnpm tauri dev` from `examples/tauri-app/` and verify the golden path:
   - Generate key → pubkey appears
   - Publish Setting → log shows ✓
   - Fetch Setting → inputs update
   - Poll for Updates → log shows count (0 on first call after fetch, 1 after re-publish)

---

## What Is Explicitly Out of Scope

- Automatic relay status polling (user clicks "Get Relays" to refresh)
- Automatic poll scheduling (user clicks "Poll for Updates")
- Persisting the nsec between app restarts
- Multi-setting categories beyond `display-setting`
- Mobile layout
- Automated UI tests
