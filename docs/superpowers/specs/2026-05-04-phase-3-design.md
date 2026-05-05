# tauri-plugin-nostr-sync — Phase 3 Design

## Scope

Phase 3 adds explicit on-demand polling with frontend push notification. The host app calls `poll(categories)` on whatever schedule it chooses; the plugin returns only changed categories and emits `nostr-sync://updated` for each one.

### Explicitly out of scope

- **Outbox / retry / backoff** — cancelled. `publish` remains synchronous `Result`; transient failures are the caller's responsibility.
- **`nostr-sync://relay-status` events** — dropped for simplicity. The host app already has `get_status()`.
- **`nostr-sync://error` events** — dropped (YAGNI). All errors in a polling model have a synchronous caller; no background task exists to need a push error channel.
- **Background subscription task** — no `client.notifications()` loop. No background tasks of any kind.

---

## Decisions

- **Polling is explicit** — the host app calls `poll(categories)` on its own schedule. The plugin provides no timer, no watcher, no subscription.
- **Dedup via `last_seen` HashMap** — in-memory `category → nostr_sdk::Timestamp`. Resets on process launch (correct: after restart the first poll per category always returns a result, enabling "sync on resume").
- **`NostrSyncState` stays Tauri-free** — `poll` logic lives in `state.rs`; event emission lives in `desktop.rs` where the `AppHandle` already is. Core unit tests require no `MockRuntime`.
- **One event type: `nostr-sync://updated`** — payload is `FetchResult` (same shape as fetch/sync_all).
- **`_app` renamed to `app`** — the `TauriPluginNostrSync` struct field was prefixed with `_` to suppress unused warnings; now that it's used for `emit`, the underscore is removed.

---

## State changes (`src/state.rs`)

### New field

```rust
pub struct NostrSyncState {
    pub(crate) namespace: String,
    pub(crate) device_id: String,
    pub(crate) client: Client,
    last_seen: tokio::sync::RwLock<std::collections::HashMap<String, nostr_sdk::Timestamp>>,
}
```

Initialized to an empty map in `NostrSyncState::new`.

### Internal refactor: `build_filter`

Extract a private helper used by both `fetch` and `poll` to avoid duplication:

```rust
fn build_filter(pubkey: PublicKey, namespace: &str, category: &str) -> Filter {
    Filter::new()
        .kind(Kind::from(30078u16))
        .author(pubkey)
        .identifier(build_dtag(namespace, category))
}
```

### New method: `poll`

```rust
pub async fn poll(&self, categories: &[String]) -> Result<Vec<FetchResult>>
```

For each category:

1. `validate_category(category)?`
2. Get signer → `Error::SignerNotSet` if absent (checked once before the loop)
3. Run `build_filter(pubkey, &self.namespace, category)`
4. `client.fetch_events(filter, 10s timeout)`
5. Take `events.first()`:
   - If `None`: skip (nothing published for this category)
   - If `Some(event)`:
     - Read `last_seen.read()[category]`
     - If `event.created_at > last_seen` (or category not yet seen): update `last_seen`, decrypt, push `FetchResult` to results
     - If not newer: skip silently
6. Return `Ok(results)` — empty `Vec` is not an error

`fetch` is updated to use `build_filter` internally (no behaviour change).

---

## Tauri layer

### `desktop.rs`

Rename `_app` → `app` on `TauriPluginNostrSync<R>`. Add:

```rust
pub async fn poll(&self, categories: &[String]) -> Result<Vec<FetchResult>> {
    let updates = self.pub_state.poll(categories).await?;
    for update in &updates {
        let _ = self.app.emit("nostr-sync://updated", update);
    }
    Ok(updates)
}
```

### `models.rs`

Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollRequest {
    pub categories: Vec<String>,
}
```

The `nostr-sync://updated` event payload reuses `FetchResult` (no new type needed).

### `commands.rs`

```rust
#[tauri::command]
pub async fn poll<R: Runtime>(app: AppHandle<R>, request: PollRequest) -> Result<Vec<FetchResult>> {
    app.nostr_sync().poll(&request.categories).await
}
```

### `build.rs`

Add `"poll"` to the `COMMANDS` array.

### `permissions/`

Add a permission entry for `poll` following the pattern of the 8 Phase 2 commands.

### `guest-js/index.ts`

```typescript
export async function poll(categories: string[]): Promise<FetchResult[]> {
    return invoke<FetchResult[]>('plugin:tauri-plugin-nostr-sync|poll', {
        request: { categories },
    });
}
```

---

## Event payload

`nostr-sync://updated` carries a single `FetchResult`:

```typescript
interface FetchResult {
    category: string;
    payload: unknown;
    updatedAt: string;   // ISO 8601
    deviceId: string;
}
```

One event is emitted per changed category per `poll` call. If three categories changed, three events fire.

---

## Data flow

```
host app (JS)
  │
  │  invoke poll({ categories: ["ui-settings", "wallet"] })
  ▼
commands.rs :: poll
  │
  │  app.nostr_sync().poll(categories)
  ▼
desktop.rs :: poll
  │
  │  pub_state.poll(categories)
  ▼
state.rs :: poll
  for each category:
    fetch_events from relay
    compare created_at vs last_seen
    if newer → decrypt → push to results, update last_seen
  return Vec<FetchResult>
  │
  ◄─────────────────────────────────────────────
  │
  for each update:
    app.emit("nostr-sync://updated", update)     ← Tauri event to frontend
  return Ok(updates) to JS caller
```

---

## Testing

### Level 1 — Unit tests (in `src/state.rs`)

No new unit tests for `build_filter` (trivial helper). The `last_seen` logic is covered by Level 2.

### Level 2 — Integration tests (`tests/integration.rs`, using existing `MockRelay`)

| Test | Assertion |
|---|---|
| `poll_returns_empty_when_no_events` | Poll on unpublished category → `Ok([])` |
| `poll_returns_update_on_first_call` | Publish then poll → one `FetchResult` |
| `poll_deduplicates_unchanged_events` | Poll twice without re-publish → second poll `Ok([])` |
| `poll_returns_update_after_republish` | Publish, poll, publish newer, poll again → second poll returns update |
| `poll_with_multiple_categories_returns_only_changed` | Publish two, poll both, re-publish one, poll again → one result |
| `poll_without_signer_returns_signer_not_set` | No signer → `Err(SignerNotSet)` |

### Level 3 — Tauri command tests (`tests/commands.rs`, `MockRuntime`)

| Test | Assertion |
|---|---|
| `poll_command_requires_categories_field` | Malformed request → command error |
| `poll_command_returns_empty_vec_with_no_relay` | Well-formed request, no relay → `Ok([])` |

### Level 4 — E2E tests (`tests/e2e.rs`, `#[ignore = "requires local relay"]`)

Two `NostrSyncState` instances, shared keypair, local relay. Instance A publishes; instance B polls. Assert B's poll returns the correct payload with `device_id` matching A's `device_id`.

---

## Files modified

| File | Change |
|---|---|
| `src/state.rs` | Add `last_seen` field, `build_filter` helper, `poll` method; refactor `fetch` to use `build_filter` |
| `src/desktop.rs` | Rename `_app` → `app`; add `poll` method with event emission |
| `src/models.rs` | Add `PollRequest` |
| `src/commands.rs` | Add `poll` command |
| `build.rs` | Register `poll` in `COMMANDS` |
| `permissions/` | Add permission entry for `poll` |
| `guest-js/index.ts` | Export `poll` function |
| `dist-js/` | Rebuilt output (generated) |
| `tests/integration.rs` | Add 6 Level 2 poll tests |
| `tests/commands.rs` | Add 2 Level 3 poll command tests |
| `tests/e2e.rs` | New file: Level 4 E2E test (ignored) |
