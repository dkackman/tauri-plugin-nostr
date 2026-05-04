mod common;

use tauri_plugin_nostr_sync::{PluginBuilder, TauriPluginNostrSyncExt};

fn build_test_app() -> tauri::App<tauri::test::MockRuntime> {
    tauri::test::mock_builder()
        .plugin(PluginBuilder::new().build())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("failed to build test app")
}

#[tokio::test]
async fn get_status_returns_not_ready_before_signer() {
    let app = build_test_app();
    let status = app.nostr_sync().status().await;
    assert!(!status.ready);
    assert_eq!(status.relay_count, 0);
    assert_eq!(status.connected_relay_count, 0);
}

#[tokio::test]
async fn get_relays_returns_empty_list_initially() {
    let app = build_test_app();
    let relays = app.nostr_sync().relays().await;
    assert!(relays.is_empty());
}

#[tokio::test]
async fn get_pubkey_returns_none_before_signer() {
    let app = build_test_app();
    let pubkey = app.nostr_sync().pubkey().await;
    assert!(pubkey.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn add_and_get_relays_round_trips() {
    let relay = common::MockRelay::start().await;
    let app = build_test_app();

    app.nostr_sync().add_relay(&relay.url()).await.unwrap();
    let relays = app.nostr_sync().relays().await;
    assert_eq!(relays.len(), 1);
    assert_eq!(relays[0].url, relay.url());
    relay.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fetch_returns_none_when_no_events_exist() {
    let relay = common::MockRelay::start().await;
    let app = build_test_app();

    app.nostr_sync().add_relay(&relay.url()).await.unwrap();
    app.nostr_sync()
        .set_signer(nostr_sdk::Keys::generate())
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let result = app.nostr_sync().fetch("ui-settings").await.unwrap();
    assert!(result.is_none());
    relay.shutdown().await;
}
