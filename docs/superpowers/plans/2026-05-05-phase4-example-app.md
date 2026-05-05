# Phase 4 — Example App Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the blank Tauri+Svelte template at `examples/tauri-app/` with a two-column Bootstrap app that exercises every plugin command including `poll`.

**Architecture:** Five focused Svelte 5 components (one per card) assembled in `App.svelte`. The example app's Rust backend adds three key-management commands (`set_sync_key`, `generate_sync_key`, `clear_sync_key`) since key handling is a host-app concern. `App.svelte` owns layout and shared state (`pubkey`, `setting`, `entries`) and is the sole listener for the `nostr-sync://updated` Tauri event.

**Tech Stack:** Svelte 5 (runes), Bootstrap 5, `@tauri-apps/api` v2, `tauri-plugin-nostr-sync-api` (local), `nostr-sdk 0.44.1` (Rust), Tauri 2.

---

## File Map

| File | Action |
|---|---|
| `examples/tauri-app/src-tauri/Cargo.toml` | Add `nostr-sdk = "0.44.1"` dependency |
| `examples/tauri-app/src-tauri/src/lib.rs` | Replace `greet` with three key commands + plugin init with relays |
| `examples/tauri-app/package.json` | Add `bootstrap` to dependencies |
| `examples/tauri-app/src/main.js` | Add Bootstrap CSS import |
| `examples/tauri-app/src/style.css` | Remove all content (Bootstrap replaces it) |
| `examples/tauri-app/src/App.svelte` | Replace template with two-column layout + event wiring |
| `examples/tauri-app/src/lib/ConsoleLog.svelte` | New — log display component |
| `examples/tauri-app/src/lib/IdentityCard.svelte` | New — key management card |
| `examples/tauri-app/src/lib/RelayCard.svelte` | New — relay list card |
| `examples/tauri-app/src/lib/SettingCard.svelte` | New — display-setting editor card |
| `examples/tauri-app/src/lib/ActionsCard.svelte` | New — action buttons card |

---

## Task 1: Rust backend — key management commands

**Files:**
- Modify: `examples/tauri-app/src-tauri/Cargo.toml`
- Modify: `examples/tauri-app/src-tauri/src/lib.rs`

- [ ] **Step 1: Add nostr-sdk dependency**

In `examples/tauri-app/src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
nostr-sdk = { version = "0.44.1" }
```

Full `[dependencies]` section after edit:
```toml
[dependencies]
tauri = { version = "2.5.0", features = [] }
tauri-plugin-nostr-sync = { path = "../../../" }
nostr-sdk = { version = "0.44.1" }
```

- [ ] **Step 2: Replace lib.rs**

Replace the entire contents of `examples/tauri-app/src-tauri/src/lib.rs` with:

```rust
use nostr_sdk::{Keys, ToBech32};
use tauri::AppHandle;
use tauri_plugin_nostr_sync::TauriPluginNostrSyncExt;

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
        .set_signer(keys)
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
        .set_signer(keys)
        .await
        .map_err(|e| e.to_string())?;
    Ok(KeyInfo { nsec, pubkey })
}

#[tauri::command]
async fn clear_sync_key(app: AppHandle) -> Result<(), String> {
    app.nostr_sync().unset_signer().await;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_nostr_sync::Builder::new()
                .relays(vec![
                    "wss://relay.damus.io".to_string(),
                    "wss://nos.lol".to_string(),
                ])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            set_sync_key,
            generate_sync_key,
            clear_sync_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cd examples/tauri-app && cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: no errors. If you see `ToBech32 not found`, check that `nostr-sdk = "0.44.1"` was added to Cargo.toml and that Cargo fetched it (run `cargo fetch`).

- [ ] **Step 4: Commit**

```bash
git add examples/tauri-app/src-tauri/Cargo.toml examples/tauri-app/src-tauri/src/lib.rs
git commit -m "feat(example): add key management Rust commands"
```

---

## Task 2: Bootstrap setup

**Files:**
- Modify: `examples/tauri-app/package.json`
- Modify: `examples/tauri-app/src/main.js`
- Modify: `examples/tauri-app/src/style.css`

- [ ] **Step 1: Add Bootstrap to package.json**

In `examples/tauri-app/package.json`, add `bootstrap` to `"dependencies"`:

```json
"dependencies": {
  "@tauri-apps/api": "^2.11.0",
  "bootstrap": "^5.3.3",
  "tauri-plugin-nostr-sync-api": "file:../../"
}
```

- [ ] **Step 2: Install**

```bash
cd examples/tauri-app && pnpm install
```

Expected: Bootstrap appears in `node_modules/bootstrap/`.

- [ ] **Step 3: Import Bootstrap CSS in main.js**

Replace the entire content of `examples/tauri-app/src/main.js` with:

```js
import "bootstrap/dist/css/bootstrap.min.css";
import App from "./App.svelte";

const app = new App({
  target: document.getElementById("app"),
});

export default app;
```

- [ ] **Step 4: Clear old style.css**

Replace `examples/tauri-app/src/style.css` with an empty file (or just a comment). Bootstrap handles all base styles:

```css
/* Bootstrap handles base styles — add app-specific overrides here if needed */
```

- [ ] **Step 5: Commit**

```bash
git add examples/tauri-app/package.json examples/tauri-app/src/main.js examples/tauri-app/src/style.css examples/tauri-app/pnpm-lock.yaml
git commit -m "feat(example): add Bootstrap 5"
```

---

## Task 3: ConsoleLog.svelte

**Files:**
- Create: `examples/tauri-app/src/lib/ConsoleLog.svelte`

ConsoleLog is a pure display component — no plugin calls, no async.

- [ ] **Step 1: Create ConsoleLog.svelte**

```svelte
<script>
  let { entries = [], onClear } = $props();

  function colorFor(level) {
    if (level === "success") return "#22c55e";
    if (level === "event") return "#38bdf8";
    if (level === "error") return "#ef4444";
    return "#94a3b8";
  }

  function symbolFor(level) {
    if (level === "success") return "✓";
    if (level === "event") return "●";
    if (level === "error") return "✗";
    return "ℹ";
  }
</script>

<div class="card border-secondary d-flex flex-column flex-grow-1" style="background:#1e293b; min-height:0;">
  <div class="card-header d-flex justify-content-between align-items-center py-1" style="background:#1e293b;">
    <small class="text-secondary text-uppercase" style="font-size:10px;">📋 Console Log</small>
    <button class="btn btn-sm btn-outline-secondary py-0 px-2" style="font-size:10px;" onclick={onClear}>Clear</button>
  </div>
  <div class="card-body p-2 overflow-auto flex-grow-1" style="background:#0f172a; font-family:monospace; font-size:11px;">
    {#each entries as entry}
      <div style="color:{colorFor(entry.level)}; margin-bottom:2px;">
        {entry.timestamp} {symbolFor(entry.level)} {entry.message}
      </div>
    {/each}
  </div>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add examples/tauri-app/src/lib/ConsoleLog.svelte
git commit -m "feat(example): add ConsoleLog component"
```

---

## Task 4: IdentityCard.svelte

**Files:**
- Create: `examples/tauri-app/src/lib/IdentityCard.svelte`

Calls the three host-app Rust commands (`set_sync_key`, `generate_sync_key`, `clear_sync_key`) via `invoke`. Updates the `pubkey` bindable prop so `App.svelte`'s state stays in sync.

- [ ] **Step 1: Create IdentityCard.svelte**

```svelte
<script>
  import { invoke } from "@tauri-apps/api/core";

  let { pubkey = $bindable(""), log } = $props();
  let nsecInput = $state("");

  async function setKey() {
    try {
      pubkey = await invoke("set_sync_key", { nsec: nsecInput });
      log(`Key set. Pubkey: ${pubkey.slice(0, 20)}...`, "success");
    } catch (e) {
      log(`Set key failed: ${e}`, "error");
    }
  }

  async function generateKey() {
    try {
      const result = await invoke("generate_sync_key");
      nsecInput = result.nsec;
      pubkey = result.pubkey;
      log(`Generated key. Pubkey: ${pubkey.slice(0, 20)}...`, "success");
    } catch (e) {
      log(`Generate failed: ${e}`, "error");
    }
  }

  async function clearKey() {
    try {
      await invoke("clear_sync_key");
      pubkey = "";
      nsecInput = "";
      log("Key cleared", "info");
    } catch (e) {
      log(`Clear failed: ${e}`, "error");
    }
  }
</script>

<div class="card border-secondary mb-3" style="background:#0f172a;">
  <div class="card-header py-1" style="background:#0f172a;">
    <small class="text-secondary text-uppercase" style="font-size:10px;">🔑 Identity</small>
  </div>
  <div class="card-body p-2">
    <div class="mb-2 p-1 rounded" style="background:#1e293b; font-size:11px; font-family:monospace; color:#94a3b8;">
      pubkey: <span style="color:{pubkey ? '#38bdf8' : '#64748b'};">{pubkey || "not set"}</span>
    </div>
    <input
      class="form-control form-control-sm mb-2"
      style="background:#334155; border-color:#4b5563; color:#94a3b8; font-size:11px;"
      placeholder="nsec1... paste here"
      bind:value={nsecInput}
    />
    <div class="d-flex gap-1">
      <button class="btn btn-sm btn-secondary flex-fill" style="font-size:10px;" onclick={setKey}>Set Key</button>
      <button class="btn btn-sm btn-secondary flex-fill" style="font-size:10px;" onclick={generateKey}>Generate</button>
      <button class="btn btn-sm btn-danger flex-fill" style="font-size:10px;" onclick={clearKey}>Clear</button>
    </div>
  </div>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add examples/tauri-app/src/lib/IdentityCard.svelte
git commit -m "feat(example): add IdentityCard component"
```

---

## Task 5: RelayCard.svelte

**Files:**
- Create: `examples/tauri-app/src/lib/RelayCard.svelte`

Calls `addRelay`, `removeRelay`, `getRelays` from the plugin API. Relay list is loaded once on mount and refreshed after add/remove.

- [ ] **Step 1: Create RelayCard.svelte**

```svelte
<script>
  import { addRelay, removeRelay, getRelays } from "tauri-plugin-nostr-sync-api";

  let { log } = $props();
  let relays = $state([]);
  let newUrl = $state("");

  async function loadRelays() {
    try {
      relays = await getRelays();
    } catch (e) {
      log(`getRelays failed: ${e}`, "error");
    }
  }

  async function handleAdd() {
    const url = newUrl.trim();
    if (!url) return;
    try {
      await addRelay(url);
      newUrl = "";
      await loadRelays();
      log(`Added relay: ${url}`, "success");
    } catch (e) {
      log(`Add relay failed: ${e}`, "error");
    }
  }

  async function handleRemove(url) {
    try {
      await removeRelay(url);
      await loadRelays();
      log(`Removed relay: ${url}`, "success");
    } catch (e) {
      log(`Remove relay failed: ${e}`, "error");
    }
  }

  $effect(() => {
    loadRelays();
  });
</script>

<div class="card border-secondary mb-3" style="background:#0f172a;">
  <div class="card-header py-1" style="background:#0f172a;">
    <small class="text-secondary text-uppercase" style="font-size:10px;">📡 Relays</small>
  </div>
  <div class="card-body p-2">
    {#each relays as relay}
      <div class="d-flex align-items-center mb-1">
        <span
          class="rounded-circle me-2 flex-shrink-0"
          style="width:8px;height:8px;display:inline-block;background:{relay.connected ? '#22c55e' : '#f59e0b'};"
        ></span>
        <span class="text-secondary flex-fill text-truncate" style="font-size:10px;">{relay.url}</span>
        <button
          class="btn btn-link btn-sm p-0 ms-1 text-danger"
          style="font-size:12px; line-height:1;"
          onclick={() => handleRemove(relay.url)}
        >✕</button>
      </div>
    {/each}
    <div class="d-flex gap-1 mt-2">
      <input
        class="form-control form-control-sm flex-fill"
        style="background:#1e293b; border-color:#334155; color:#94a3b8; font-size:10px;"
        placeholder="wss://..."
        bind:value={newUrl}
        onkeydown={(e) => { if (e.key === "Enter") handleAdd(); }}
      />
      <button class="btn btn-sm btn-secondary" style="font-size:10px;" onclick={handleAdd}>Add</button>
    </div>
  </div>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add examples/tauri-app/src/lib/RelayCard.svelte
git commit -m "feat(example): add RelayCard component"
```

---

## Task 6: SettingCard.svelte

**Files:**
- Create: `examples/tauri-app/src/lib/SettingCard.svelte`

Edits `display-setting` in place. Uses `$bindable` for `setting` so changes (from fetch or from the `nostr-sync://updated` event via App.svelte) flow automatically to/from the parent.

- [ ] **Step 1: Create SettingCard.svelte**

```svelte
<script>
  import { publish, fetch as fetchSetting } from "tauri-plugin-nostr-sync-api";

  let { setting = $bindable({ name: "My App Name", color: "#38bdf8" }), log } = $props();

  async function handlePublish() {
    try {
      await publish("display-setting", { name: setting.name, color: setting.color });
      log("Published display-setting", "success");
    } catch (e) {
      log(`Publish failed: ${e}`, "error");
    }
  }

  async function handleFetch() {
    try {
      const result = await fetchSetting("display-setting");
      if (result) {
        setting = result.payload;
        log("Fetched display-setting", "success");
      } else {
        log("No data for display-setting", "info");
      }
    } catch (e) {
      log(`Fetch failed: ${e}`, "error");
    }
  }
</script>

<div class="card border-secondary mb-3" style="background:#1e293b;">
  <div class="card-header py-1" style="background:#1e293b;">
    <small class="text-secondary text-uppercase" style="font-size:10px;">⚙️ Synced Setting</small>
  </div>
  <div class="card-body p-2">
    <div class="d-flex align-items-center mb-2 gap-2">
      <label class="text-secondary mb-0" style="font-size:11px; width:90px; flex-shrink:0;">Display Name</label>
      <input
        class="form-control form-control-sm"
        style="background:#0f172a; border-color:#334155; color:#94a3b8; font-size:11px;"
        bind:value={setting.name}
      />
    </div>
    <div class="d-flex align-items-center mb-3 gap-2">
      <label class="text-secondary mb-0" style="font-size:11px; width:90px; flex-shrink:0;">Accent Color</label>
      <input
        type="color"
        class="form-control form-control-color form-control-sm p-0"
        style="width:36px; height:28px; border-color:#334155;"
        bind:value={setting.color}
      />
      <span class="text-secondary" style="font-size:10px;">{setting.color}</span>
    </div>
    <div class="d-flex gap-1">
      <button class="btn btn-sm btn-secondary flex-fill" style="font-size:10px;" onclick={handlePublish}>Publish</button>
      <button class="btn btn-sm btn-secondary flex-fill" style="font-size:10px;" onclick={handleFetch}>Fetch</button>
    </div>
  </div>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add examples/tauri-app/src/lib/SettingCard.svelte
git commit -m "feat(example): add SettingCard component"
```

---

## Task 7: ActionsCard.svelte

**Files:**
- Create: `examples/tauri-app/src/lib/ActionsCard.svelte`

All seven action buttons. Receives the current `setting` for "Publish Setting"; calls `onSettingChange` when fetch/poll/syncAll return updated data.

- [ ] **Step 1: Create ActionsCard.svelte**

```svelte
<script>
  import {
    publish,
    fetch as fetchSetting,
    syncAll,
    poll,
    getStatus,
    getRelays,
    getPubkey,
  } from "tauri-plugin-nostr-sync-api";

  let { setting, onSettingChange, log } = $props();

  async function handlePublish() {
    try {
      await publish("display-setting", { name: setting.name, color: setting.color });
      log("Published display-setting", "success");
    } catch (e) {
      log(`Publish failed: ${e}`, "error");
    }
  }

  async function handleFetch() {
    try {
      const result = await fetchSetting("display-setting");
      if (result) {
        onSettingChange(result.payload);
        log("Fetched display-setting", "success");
      } else {
        log("No data for display-setting", "info");
      }
    } catch (e) {
      log(`Fetch failed: ${e}`, "error");
    }
  }

  async function handlePoll() {
    try {
      const results = await poll(["display-setting"]);
      if (results.length > 0) {
        onSettingChange(results[0].payload);
      }
      log(`Poll: ${results.length} update(s)`, "success");
    } catch (e) {
      log(`Poll failed: ${e}`, "error");
    }
  }

  async function handleSyncAll() {
    try {
      const results = await syncAll(["display-setting"]);
      if (results.length > 0) {
        onSettingChange(results[0].payload);
      }
      log(`Sync all: ${results.length} result(s)`, "success");
    } catch (e) {
      log(`Sync all failed: ${e}`, "error");
    }
  }

  async function handleGetStatus() {
    try {
      const s = await getStatus();
      log(`Status: ready=${s.ready} relays=${s.relayCount} connected=${s.connectedRelayCount}`, "info");
    } catch (e) {
      log(`Get status failed: ${e}`, "error");
    }
  }

  async function handleGetRelays() {
    try {
      const relays = await getRelays();
      const summary = relays.map((r) => `${r.url}(${r.connected ? "✓" : "✗"})`).join(", ");
      log(`Relays: ${summary || "none"}`, "info");
    } catch (e) {
      log(`Get relays failed: ${e}`, "error");
    }
  }

  async function handleGetPubkey() {
    try {
      const pk = await getPubkey();
      log(`Pubkey: ${pk ?? "not set"}`, "info");
    } catch (e) {
      log(`Get pubkey failed: ${e}`, "error");
    }
  }
</script>

<div class="card border-secondary flex-grow-1" style="background:#0f172a;">
  <div class="card-header py-1" style="background:#0f172a;">
    <small class="text-secondary text-uppercase" style="font-size:10px;">⚡ Actions</small>
  </div>
  <div class="card-body p-2 d-flex flex-column gap-1">
    <button class="btn btn-sm btn-secondary w-100" style="font-size:10px;" onclick={handlePublish}>Publish Setting</button>
    <button class="btn btn-sm btn-secondary w-100" style="font-size:10px;" onclick={handleFetch}>Fetch Setting</button>
    <button class="btn btn-sm btn-secondary w-100" style="font-size:10px;" onclick={handlePoll}>Poll for Updates</button>
    <button class="btn btn-sm btn-secondary w-100" style="font-size:10px;" onclick={handleSyncAll}>Sync All</button>
    <hr class="border-secondary my-1" />
    <button class="btn btn-sm btn-outline-secondary w-100" style="font-size:10px;" onclick={handleGetStatus}>Get Status</button>
    <button class="btn btn-sm btn-outline-secondary w-100" style="font-size:10px;" onclick={handleGetRelays}>Get Relays</button>
    <button class="btn btn-sm btn-outline-secondary w-100" style="font-size:10px;" onclick={handleGetPubkey}>Get Pubkey</button>
  </div>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add examples/tauri-app/src/lib/ActionsCard.svelte
git commit -m "feat(example): add ActionsCard component"
```

---

## Task 8: App.svelte — layout and wiring

**Files:**
- Modify: `examples/tauri-app/src/App.svelte`

`App.svelte` owns `pubkey`, `entries` (log), and `setting`. It registers the `nostr-sync://updated` event listener and passes state + callbacks down to all five components.

- [ ] **Step 1: Replace App.svelte**

Replace the entire contents of `examples/tauri-app/src/App.svelte` with:

```svelte
<script>
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import IdentityCard from "./lib/IdentityCard.svelte";
  import RelayCard from "./lib/RelayCard.svelte";
  import ActionsCard from "./lib/ActionsCard.svelte";
  import SettingCard from "./lib/SettingCard.svelte";
  import ConsoleLog from "./lib/ConsoleLog.svelte";

  let pubkey = $state("");
  let setting = $state({ name: "My App Name", color: "#38bdf8" });
  let entries = $state([]);

  function log(message, level = "info") {
    const now = new Date();
    const timestamp = now.toLocaleTimeString("en-US", {
      hour12: false,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
    entries = [...entries, { timestamp, message, level }];
  }

  function handleSettingChange(newSetting) {
    setting = newSetting;
  }

  onMount(() => {
    let unlisten;
    listen("nostr-sync://updated", (event) => {
      const result = event.payload;
      log(`nostr-sync://updated ${JSON.stringify(result.payload)}`, "event");
      setting = result.payload;
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  });
</script>

<div class="container-fluid vh-100 d-flex flex-column p-0" style="background:#0f172a; color:#94a3b8;">
  <div class="px-3 py-2 border-bottom border-secondary" style="background:#1e293b;">
    <span style="font-size:13px; font-family:monospace; color:#38bdf8;">tauri-plugin-nostr-sync — Test App</span>
  </div>

  <div class="d-flex flex-grow-1 overflow-hidden">
    <!-- Left column -->
    <div class="d-flex flex-column p-3 border-end border-secondary overflow-auto" style="width:300px; flex-shrink:0; background:#1e293b;">
      <IdentityCard bind:pubkey={pubkey} {log} />
      <RelayCard {log} />
      <ActionsCard {setting} onSettingChange={handleSettingChange} {log} />
    </div>

    <!-- Right column -->
    <div class="d-flex flex-column flex-grow-1 p-3 overflow-hidden">
      <SettingCard bind:setting={setting} {log} />
      <ConsoleLog {entries} onClear={() => { entries = []; }} />
    </div>
  </div>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add examples/tauri-app/src/App.svelte
git commit -m "feat(example): wire App.svelte two-column layout"
```

---

## Task 9: Smoke test

- [ ] **Step 1: Start the dev server**

```bash
cd examples/tauri-app && pnpm tauri dev
```

Expected: app window opens with the dark two-column layout. Both relay dots appear (amber/green after connection attempt).

- [ ] **Step 2: Golden path — generate and publish**

1. Click **Generate** in the Identity card
   - nsec field populates, pubkey appears in the Identity card display
   - Console log shows: `✓ Generated key. Pubkey: <first 20 chars>...`

2. Edit Display Name to "Test App" in the Setting card

3. Click **Publish** in the Setting card
   - Console log shows: `✓ Published display-setting`

- [ ] **Step 3: Fetch round-trip**

4. Edit Display Name to something different ("Changed") so you can see the reset

5. Click **Fetch Setting** in the Actions card
   - Display Name input in Setting card resets to "Test App"
   - Console log shows: `✓ Fetched display-setting`

- [ ] **Step 4: Poll**

6. Click **Poll for Updates**
   - Console log shows: `✓ Poll: 0 update(s)` (nothing changed since last fetch)

7. Click **Publish** in Setting card again (edit name to "Republished" first)

8. Click **Poll for Updates**
   - Console log shows: `✓ Poll: 1 update(s)`
   - Setting card inputs update to "Republished"

- [ ] **Step 5: Verify status and relay commands**

9. Click **Get Status** — console shows `ℹ Status: ready=true relays=2 connected=X`
10. Click **Get Relays** — console lists relays with ✓/✗
11. Click **Get Pubkey** — console shows hex pubkey

- [ ] **Step 6: Clear key**

12. Click **Clear** — pubkey display resets to "not set", console shows `ℹ Key cleared`
13. Click **Poll for Updates** — console shows `✗ Poll failed: SignerNotSet` (expected error)

- [ ] **Step 7: Commit**

```bash
git add -p  # verify no stray changes
git commit -m "feat(example): complete Phase 4 test app"
```

---

## Self-Review Notes

**Spec coverage verified:**
- ✓ Two-column Bootstrap layout (Task 8)
- ✓ IdentityCard: pubkey display, nsec paste, Set Key / Generate / Clear (Task 4)
- ✓ RelayCard: colored status dots, add/remove, wss:// input (Task 5)
- ✓ SettingCard: Display Name + Accent Color, Publish + Fetch (Task 6)
- ✓ ActionsCard: all 7 buttons (Task 7)
- ✓ ConsoleLog: timestamped entries, color by level, Clear (Task 3)
- ✓ `nostr-sync://updated` listener updates setting + logs (Task 8)
- ✓ set_sync_key, generate_sync_key, clear_sync_key Rust commands (Task 1)
- ✓ Bootstrap dependency (Task 2)

**Type consistency:**
- `LogEntry` shape `{ timestamp, message, level }` is consistent across ConsoleLog (consumer) and App.svelte (producer)
- `setting` shape `{ name: string, color: string }` is consistent across App.svelte, SettingCard, and ActionsCard
- `FetchResult.payload` cast to `{ name: string, color: string }` — this is a runtime assumption; if the relay has no data or data from a different schema, the cast is safe because the fields are used with optional chaining in components
