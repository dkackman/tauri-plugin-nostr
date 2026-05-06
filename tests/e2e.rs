use tauri_plugin_nostr_sync::NostrSyncState;

/// Requires a local Nostr relay listening at ws://127.0.0.1:7777.
/// Start one with: docker run -p 7777:8080 scsibug/nostr-rs-relay
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires local relay"]
async fn two_instances_publish_and_poll() {
    let relay_url = "ws://127.0.0.1:7777";
    let keys = nostr_sdk::Keys::generate();

    // Instance A publishes
    let sender = NostrSyncState::new("e2e-test", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    sender.add_relay(relay_url).await.unwrap();
    sender.set_signer(keys.clone()).await.unwrap();
    sender
        .wait_for_connection(std::time::Duration::from_secs(10))
        .await;

    let payload = serde_json::json!({"value": "hello-from-a"});
    sender.publish("settings", &payload, None).await.unwrap();

    // Instance B polls (same keypair — same pubkey, can decrypt NIP-44)
    let receiver = NostrSyncState::new("e2e-test", "test-device", tauri_plugin_nostr_sync::DEFAULT_PAYLOAD_LIMIT).unwrap();
    receiver.add_relay(relay_url).await.unwrap();
    receiver.set_signer(keys.clone()).await.unwrap();
    receiver
        .wait_for_connection(std::time::Duration::from_secs(10))
        .await;

    let updates = receiver.poll(&["settings".to_string()]).await.unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].category, "settings");
    assert_eq!(updates[0].payload, payload);
    assert!(!updates[0].device_id.is_empty());
}
