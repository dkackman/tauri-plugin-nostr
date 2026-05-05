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

  let { setting = { name: "", color: "" }, onSettingChange, log } = $props();

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
      log(
        `Status: ready=${s.ready} relays=${s.relayCount} connected=${s.connectedRelayCount}`,
        "info"
      );
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
    <button class="btn btn-sm btn-secondary w-100" style="font-size:10px;" onclick={handlePublish}
      >Publish Setting</button
    >
    <button class="btn btn-sm btn-secondary w-100" style="font-size:10px;" onclick={handleFetch}
      >Fetch Setting</button
    >
    <button class="btn btn-sm btn-secondary w-100" style="font-size:10px;" onclick={handlePoll}
      >Poll for Updates</button
    >
    <button class="btn btn-sm btn-secondary w-100" style="font-size:10px;" onclick={handleSyncAll}
      >Sync All</button
    >
    <hr class="border-secondary my-1" />
    <button
      class="btn btn-sm btn-outline-secondary w-100"
      style="font-size:10px;"
      onclick={handleGetStatus}>Get Status</button
    >
    <button
      class="btn btn-sm btn-outline-secondary w-100"
      style="font-size:10px;"
      onclick={handleGetRelays}>Get Relays</button
    >
    <button
      class="btn btn-sm btn-outline-secondary w-100"
      style="font-size:10px;"
      onclick={handleGetPubkey}>Get Pubkey</button
    >
  </div>
</div>
