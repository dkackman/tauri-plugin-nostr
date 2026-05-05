<script>
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getStatus } from "tauri-plugin-nostr-sync-api";
  import IdentityCard from "./lib/IdentityCard.svelte";
  import RelayCard from "./lib/RelayCard.svelte";
  import ActionsCard from "./lib/ActionsCard.svelte";
  import SettingCard from "./lib/SettingCard.svelte";
  import ConsoleLog from "./lib/ConsoleLog.svelte";

  let pubkey = $state("");
  let darkMode = $state(false);
  let ownDeviceId = $state("");
  let entries = $state([]);

  // Keep Bootstrap's color mode in sync with darkMode state.
  $effect(() => {
    document.documentElement.setAttribute("data-bs-theme", darkMode ? "dark" : "light");
  });

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

  onMount(async () => {
    // Fetch our own device ID once so we can filter our own published events.
    try {
      const status = await getStatus();
      ownDeviceId = status.deviceId;
    } catch (e) {
      log(`Could not fetch device ID: ${e}`, "error");
    }

    const unlisten = await listen("nostr-sync://updated", (event) => {
      const result = event.payload;

      // Skip events that originated from this instance — we already applied them locally.
      if (result.deviceId === ownDeviceId) {
        log(`Ignored own update (${result.category})`, "info");
        return;
      }

      const dm = result.payload.darkMode ?? false;
      log(
        `Received update from ${result.deviceId.slice(0, 8)}…: dark mode ${dm ? "on" : "off"}`,
        "event"
      );
      darkMode = dm;
    });
    return () => unlisten();
  });
</script>

<div class="container-fluid vh-100 d-flex flex-column p-0 bg-body text-body">
  <div class="px-3 py-2 border-bottom bg-body-secondary">
    <span style="font-size:13px; font-family:monospace;" class="text-primary"
      >tauri-plugin-nostr-sync — Test App</span
    >
  </div>

  <div class="d-flex flex-grow-1 overflow-hidden">
    <!-- Left column -->
    <div
      class="d-flex flex-column p-3 border-end overflow-auto bg-body-secondary"
      style="width:300px; flex-shrink:0;"
    >
      <IdentityCard bind:pubkey {log} />
      <RelayCard {log} />
      <ActionsCard {darkMode} onDarkModeChange={(v) => (darkMode = v)} {log} />
    </div>

    <!-- Right column -->
    <div class="d-flex flex-column flex-grow-1 p-3 overflow-hidden">
      <SettingCard bind:darkMode {log} />
      <ConsoleLog
        {entries}
        onClear={() => {
          entries = [];
        }}
      />
    </div>
  </div>
</div>
