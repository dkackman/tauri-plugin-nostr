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

  onMount(async () => {
    const unlisten = await listen("nostr-sync://updated", (event) => {
      const result = event.payload;
      log(`nostr-sync://updated ${JSON.stringify(result.payload)}`, "event");
      setting = result.payload;
    });
    return () => unlisten();
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
