mod common;

use tauri_plugin_nostr_sync::{Error, NostrSyncState};

fn make_keys() -> nostr_sdk::Keys {
    nostr_sdk::Keys::generate()
}

// ── Error-surface tests (no relay needed) ─────────────────────────────────

#[tokio::test]
async fn publish_without_signer_returns_signer_not_set() {
    let state = NostrSyncState::new("testapp").unwrap();
    let result = state
        .publish("ui-settings", &serde_json::json!({"x": 1}))
        .await;
    assert!(matches!(result, Err(Error::SignerNotSet)));
}

#[tokio::test]
async fn fetch_without_signer_returns_signer_not_set() {
    let state = NostrSyncState::new("testapp").unwrap();
    let result = state.fetch("ui-settings").await;
    assert!(matches!(result, Err(Error::SignerNotSet)));
}

#[tokio::test]
async fn sync_all_without_signer_returns_signer_not_set() {
    let state = NostrSyncState::new("testapp").unwrap();
    let result = state.sync_all(&["ui-settings".to_string()]).await;
    assert!(matches!(result, Err(Error::SignerNotSet)));
}

#[tokio::test]
async fn payload_at_64kb_limit_accepted() {
    let state = NostrSyncState::new("testapp").unwrap();
    state.set_signer(make_keys()).await.unwrap();
    // A JSON string value serializes with enclosing quotes: N chars → N+2 bytes.
    // Use 64*1024 - 2 chars so the serialized form is exactly at the 64KB limit.
    let big = "x".repeat(64 * 1024 - 2);
    let payload = serde_json::json!(big);
    // publish will fail at send_event (no relay), but the size check passes first
    let result = state.publish("big", &payload).await;
    assert!(!matches!(result, Err(Error::PayloadTooLarge { .. })));
}

#[tokio::test]
async fn payload_over_64kb_limit_rejected() {
    let state = NostrSyncState::new("testapp").unwrap();
    state.set_signer(make_keys()).await.unwrap();
    let big = "x".repeat(64 * 1024 + 1);
    let payload = serde_json::json!(big);
    let result = state.publish("big", &payload).await;
    assert!(matches!(result, Err(Error::PayloadTooLarge { .. })));
}

// ── Relay round-trip tests ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fetch_returns_none_when_no_events_exist() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp").unwrap();
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
    let state = NostrSyncState::new("testapp").unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    let payload = serde_json::json!({"theme": "dark", "fontSize": 14});
    state.publish("ui-settings", &payload).await.unwrap();

    let result = state.fetch("ui-settings").await.unwrap();
    assert_eq!(result.unwrap().payload, payload);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn publish_with_relay_down_returns_no_relays_accepted() {
    let state = NostrSyncState::new("testapp").unwrap();
    // Add a URL where nothing is listening.
    state.add_relay("ws://127.0.0.1:19999").await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    // Give the client a moment to attempt connection (and fail).
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let result = state
        .publish("ui-settings", &serde_json::json!({"x": 1}))
        .await;
    assert!(matches!(result, Err(Error::NoRelaysAccepted)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_all_returns_all_fetched_categories() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp").unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    state.publish("ui-settings", &serde_json::json!({"theme": "dark"})).await.unwrap();
    state.publish("wallet", &serde_json::json!({"network": "mainnet"})).await.unwrap();

    let categories = vec!["ui-settings".to_string(), "wallet".to_string()];
    let results = state.sync_all(&categories).await.unwrap();
    assert_eq!(results.len(), 2);
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_all_omits_categories_with_no_data() {
    let relay = common::MockRelay::start().await;
    let state = NostrSyncState::new("testapp").unwrap();
    state.add_relay(&relay.url()).await.unwrap();
    state.set_signer(make_keys()).await.unwrap();
    state
        .wait_for_connection(std::time::Duration::from_secs(5))
        .await;

    state.publish("ui-settings", &serde_json::json!({"theme": "dark"})).await.unwrap();
    // "wallet" is never published

    let categories = vec!["ui-settings".to_string(), "wallet".to_string()];
    let results = state.sync_all(&categories).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].payload, serde_json::json!({"theme": "dark"}));
    relay.shutdown().await;
}
