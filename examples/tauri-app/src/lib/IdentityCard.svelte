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
    <div
      class="mb-2 p-1 rounded"
      style="background:#1e293b; font-size:11px; font-family:monospace; color:#94a3b8;"
    >
      pubkey: <span style="color:{pubkey ? '#38bdf8' : '#64748b'};">{pubkey || "not set"}</span>
    </div>
    <input
      class="form-control form-control-sm mb-2"
      style="background:#334155; border-color:#4b5563; color:#94a3b8; font-size:11px;"
      placeholder="nsec1... paste here"
      bind:value={nsecInput}
    />
    <div class="d-flex gap-1">
      <button class="btn btn-sm btn-secondary flex-fill" style="font-size:10px;" onclick={setKey}
        >Set Key</button
      >
      <button
        class="btn btn-sm btn-secondary flex-fill"
        style="font-size:10px;"
        onclick={generateKey}>Generate</button
      >
      <button class="btn btn-sm btn-danger flex-fill" style="font-size:10px;" onclick={clearKey}
        >Clear</button
      >
    </div>
  </div>
</div>
