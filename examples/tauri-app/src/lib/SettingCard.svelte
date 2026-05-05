<script>
  import { publish, fetch as fetchSetting } from "tauri-plugin-nostr-sync-api";

  let { darkMode = $bindable(false), onDarkModeChange, log } = $props();

  async function handleToggle() {
    try {
      await publish("display-setting", { darkMode });
      log(`Published: dark mode ${darkMode ? "on" : "off"}`, "success");
    } catch (e) {
      log(`Publish failed: ${e}`, "error");
    }
  }

  async function handlePublish() {
    try {
      await publish("display-setting", { darkMode });
      log(`Published: dark mode ${darkMode ? "on" : "off"}`, "success");
    } catch (e) {
      log(`Publish failed: ${e}`, "error");
    }
  }

  async function handleFetch() {
    try {
      const result = await fetchSetting("display-setting");
      if (result) {
        onDarkModeChange(result.payload.darkMode ?? false);
        log(`Fetched: dark mode ${result.payload.darkMode ? "on" : "off"}`, "success");
      } else {
        log("No data for display-setting", "info");
      }
    } catch (e) {
      log(`Fetch failed: ${e}`, "error");
    }
  }
</script>

<div class="card mb-3">
  <div class="card-header py-1">
    <small class="text-secondary text-uppercase" style="font-size:10px;">⚙️ Synced Setting</small>
  </div>
  <div class="card-body p-2">
    <div class="d-flex align-items-center justify-content-between mb-3">
      <label for="dark-mode-toggle" class="form-label mb-0" style="font-size:11px;">
        Dark mode
      </label>
      <div class="form-check form-switch mb-0">
        <input
          id="dark-mode-toggle"
          class="form-check-input"
          type="checkbox"
          role="switch"
          bind:checked={darkMode}
          oninput={handleToggle}
          style="cursor:pointer;"
        />
      </div>
    </div>
    <div class="d-flex gap-1">
      <button
        class="btn btn-sm btn-secondary flex-fill"
        style="font-size:10px;"
        onclick={handlePublish}>Publish</button
      >
      <button
        class="btn btn-sm btn-secondary flex-fill"
        style="font-size:10px;"
        onclick={handleFetch}>Fetch</button
      >
    </div>
  </div>
</div>
