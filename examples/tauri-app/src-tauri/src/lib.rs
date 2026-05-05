use nostr_sdk::{Keys, ToBech32};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_nostr_sync::TauriPluginNostrSyncExt;

#[derive(Serialize)]
struct KeyInfo {
    nsec: String,
    pubkey: String,
}

#[tauri::command]
async fn set_sync_key(app: AppHandle, nsec: String) -> Result<String, String> {
    let keys = Keys::parse(&nsec).map_err(|e| e.to_string())?;
    let pubkey = keys.public_key().to_hex();
    app.nostr_sync()
        .set_signer(keys)
        .await
        .map_err(|e| e.to_string())?;
    Ok(pubkey)
}

#[tauri::command]
async fn generate_sync_key(app: AppHandle) -> Result<KeyInfo, String> {
    let keys = Keys::generate();
    let nsec = keys.secret_key().to_bech32().map_err(|e| e.to_string())?;
    let pubkey = keys.public_key().to_hex();
    app.nostr_sync()
        .set_signer(keys)
        .await
        .map_err(|e| e.to_string())?;
    Ok(KeyInfo { nsec, pubkey })
}

#[tauri::command]
async fn clear_sync_key(app: AppHandle) -> Result<(), String> {
    app.nostr_sync().clear_signer().await;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            // No .device_id() call → ephemeral UUID per process, sufficient for this demo.
            // A real app would call .device_id(stable_device_id(&app)) after app setup.
            tauri_plugin_nostr_sync::Builder::new()
                .relays(vec![
                    "wss://relay.damus.io".to_string(),
                    "wss://nos.lol".to_string(),
                ])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            set_sync_key,
            generate_sync_key,
            clear_sync_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
