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
