mod common;

use tauri_plugin_nostr_sync::{Error, NostrSyncState};

fn make_keys() -> nostr_sdk::Keys {
    nostr_sdk::Keys::generate()
}

// ── Error-surface tests (no relay needed) ─────────────────────────────────

#[tokio::test]
async fn publish_without_signer_returns_signer_not_set() {
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    let result = state
        .publish("ui-settings", &serde_json::json!({"x": 1}), None)
        .await;
    assert!(matches!(result, Err(Error::SignerNotSet)));
}

#[tokio::test]
async fn fetch_without_signer_returns_signer_not_set() {
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    let result = state.fetch("ui-settings").await;
    assert!(matches!(result, Err(Error::SignerNotSet)));
}

#[tokio::test]
async fn sync_all_without_signer_returns_signer_not_set() {
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    let result = state.sync_all(&["ui-settings".to_string()]).await;
    assert!(matches!(result, Err(Error::SignerNotSet)));
}

#[tokio::test]
async fn payload_at_64kb_limit_accepted() {
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.set_signer(make_keys()).await.unwrap();
    // A JSON string value serializes with enclosing quotes: N chars → N+2 bytes.
    // Use 64*1024 - 2 chars so the serialized form is exactly at the 64KB limit.
    let big = "x".repeat(64 * 1024 - 2);
    let payload = serde_json::json!(big);
    // publish will fail at send_event (no relay), but the size check passes first
    let result = state.publish("big", &payload, None).await;
    assert!(!matches!(result, Err(Error::PayloadTooLarge { .. })));
}

#[tokio::test]
async fn payload_over_64kb_limit_rejected() {
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.set_signer(make_keys()).await.unwrap();
    let big = "x".repeat(64 * 1024 + 1);
    let payload = serde_json::json!(big);
    let result = state.publish("big", &payload, None).await;
    assert!(matches!(result, Err(Error::PayloadTooLarge { .. })));
}

// ── Relay round-trip tests ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fetch_returns_none_when_no_events_exist() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    let result = state.fetch("ui-settings").await.unwrap();
    assert!(result.is_none());
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn publish_then_fetch_returns_same_payload() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    let payload = serde_json::json!({"theme": "dark", "fontSize": 14});
    state.publish("ui-settings", &payload, None).await.unwrap();

    let result = state.fetch("ui-settings").await.unwrap();
    assert_eq!(result.unwrap().payload, payload);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn publish_with_relay_down_returns_no_relays_accepted() {
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    // Add a URL where nothing is listening.
    state.add_relay("ws://127.0.0.1:19999").await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    // Give the client a moment to attempt connection (and fail).
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let result = state
        .publish("ui-settings", &serde_json::json!({"x": 1}), None)
        .await;
    assert!(matches!(result, Err(Error::NoRelaysAccepted)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_all_returns_all_fetched_categories() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    state
        .publish("ui-settings", &serde_json::json!({"theme": "dark"}), None)
        .await
        .unwrap();
    state
        .publish("wallet", &serde_json::json!({"network": "mainnet"}), None)
        .await
        .unwrap();

    let categories = vec!["ui-settings".to_string(), "wallet".to_string()];
    let results = state.sync_all(&categories).await.unwrap();
    assert_eq!(results.len(), 2);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sequential_publishes_to_same_category_returns_latest() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    let first = serde_json::json!({"theme": "light"});
    let second = serde_json::json!({"theme": "dark"});
    state.publish("ui-settings", &first, None).await.unwrap();
    state.publish("ui-settings", &second, None).await.unwrap();

    // NIP-33 last-write-wins: the relay retains only the most recent event per d-tag.
    let result = state.fetch("ui-settings").await.unwrap();
    assert_eq!(result.unwrap().payload, second);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_all_returns_correct_payloads_per_category() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    let ui_payload = serde_json::json!({"theme": "dark"});
    let wallet_payload = serde_json::json!({"network": "mainnet"});
    state
        .publish("ui-settings", &ui_payload, None)
        .await
        .unwrap();
    state
        .publish("wallet", &wallet_payload, None)
        .await
        .unwrap();

    let categories = vec!["ui-settings".to_string(), "wallet".to_string()];
    let mut results = state.sync_all(&categories).await.unwrap();
    assert_eq!(results.len(), 2);

    // Sort by category so assertion order is deterministic.
    results.sort_by(|a, b| a.category.cmp(&b.category));
    assert_eq!(results[0].category, "ui-settings");
    assert_eq!(results[0].payload, ui_payload);
    assert_eq!(results[1].category, "wallet");
    assert_eq!(results[1].payload, wallet_payload);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_all_omits_categories_with_no_data() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    state
        .publish("ui-settings", &serde_json::json!({"theme": "dark"}), None)
        .await
        .unwrap();
    // "wallet" is never published

    let categories = vec!["ui-settings".to_string(), "wallet".to_string()];
    let results = state.sync_all(&categories).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].payload, serde_json::json!({"theme": "dark"}));
    relay.shutdown().await;
}

// ── poll tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn poll_without_signer_returns_signer_not_set() {
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    let result = state.poll(&["ui-settings".to_string()]).await;
    assert!(matches!(result, Err(Error::SignerNotSet)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn poll_returns_empty_when_no_events() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    let results = state.poll(&["ui-settings".to_string()]).await.unwrap();
    assert!(results.is_empty());
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn poll_returns_update_on_first_call() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    let payload = serde_json::json!({"theme": "dark"});
    state.publish("ui-settings", &payload, None).await.unwrap();

    let results = state.poll(&["ui-settings".to_string()]).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].category, "ui-settings");
    assert_eq!(results[0].payload, payload);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn poll_deduplicates_unchanged_events() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    let payload = serde_json::json!({"theme": "dark"});
    state.publish("ui-settings", &payload, None).await.unwrap();

    let first = state.poll(&["ui-settings".to_string()]).await.unwrap();
    assert_eq!(first.len(), 1);

    // Same event on relay — second poll must return empty
    let second = state.poll(&["ui-settings".to_string()]).await.unwrap();
    assert!(second.is_empty());
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn poll_returns_update_after_republish() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    state
        .publish("ui-settings", &serde_json::json!({"theme": "light"}), None)
        .await
        .unwrap();
    let first = state.poll(&["ui-settings".to_string()]).await.unwrap();
    assert_eq!(first.len(), 1);

    // NIP-33 timestamps have second granularity; sleep 1s so the re-publish
    // gets a strictly newer created_at and the relay replaces the old event.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let new_payload = serde_json::json!({"theme": "dark"});
    state
        .publish("ui-settings", &new_payload, None)
        .await
        .unwrap();

    let second = state.poll(&["ui-settings".to_string()]).await.unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].payload, new_payload);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn poll_with_multiple_categories_returns_only_changed() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    state
        .publish("ui-settings", &serde_json::json!({"theme": "dark"}), None)
        .await
        .unwrap();
    state
        .publish("wallet", &serde_json::json!({"network": "mainnet"}), None)
        .await
        .unwrap();

    let categories = vec!["ui-settings".to_string(), "wallet".to_string()];

    // First poll — both categories are new
    let first = state.poll(&categories).await.unwrap();
    assert_eq!(first.len(), 2);

    // Re-publish only wallet with a newer timestamp
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let updated_wallet = serde_json::json!({"network": "testnet"});
    state
        .publish("wallet", &updated_wallet, None)
        .await
        .unwrap();

    // Second poll — only wallet changed
    let second = state.poll(&categories).await.unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].category, "wallet");
    assert_eq!(second[0].payload, updated_wallet);
    relay.shutdown().await;
}
