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
