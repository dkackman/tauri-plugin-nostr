<script>
  import { invoke } from "@tauri-apps/api/core";

  let { pubkey = $bindable(""), log } = $props();
  let nsecInput = $state("");
  let copyTooltip = $state("Copy to test in another instance");

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

  async function copyNsec() {
    if (!nsecInput) return;
    try {
      await navigator.clipboard.writeText(nsecInput);
      copyTooltip = "Copied!";
      setTimeout(() => (copyTooltip = "Copy to test in another instance"), 2000);
    } catch (e) {
      log(`Copy failed: ${e}`, "error");
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

    <label for="nsec-input" class="text-secondary mb-1 d-block" style="font-size:10px;">
      Secret key (nsec)
    </label>
    <div class="d-flex gap-1 mb-2">
      <input
        id="nsec-input"
        class="form-control form-control-sm flex-fill"
        style="background:#334155; border-color:#4b5563; color:#94a3b8; font-size:11px;"
        placeholder="nsec1... paste here"
        bind:value={nsecInput}
      />
      <button
        class="btn btn-sm btn-outline-secondary"
        style="font-size:10px; padding:2px 6px;"
        title={copyTooltip}
        onclick={copyNsec}
        disabled={!nsecInput}>⎘</button
      >
    </div>

    <div
      class="mb-2 p-2 rounded"
      style="background:#1e293b; border-left:2px solid #334155; font-size:10px; color:#64748b; line-height:1.5;"
    >
      <strong style="color:#94a3b8;">What is this?</strong> The nsec is a Nostr private key that
      acts as both the signing key and the symmetric encryption secret. Any app instance that loads
      the same nsec can read and write the same encrypted sync data.<br /><br />
      <strong style="color:#94a3b8;">In a real app</strong> this key would be derived from a master secret
      (e.g. via a KDF from a user password or device key) and never shown to the user — the plugin receives
      it after the host app unlocks its own credential store.
    </div>

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
