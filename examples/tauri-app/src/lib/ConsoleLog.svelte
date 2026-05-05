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
