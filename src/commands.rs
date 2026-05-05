use tauri::{AppHandle, Runtime};

use crate::{
    FetchRequest, FetchResult, PollRequest, PublishRequest, RelayInfo, Result, SyncAllRequest,
    SyncStatus, TauriPluginNostrSyncExt,
};

#[tauri::command]
pub async fn publish<R: Runtime>(app: AppHandle<R>, request: PublishRequest) -> Result<()> {
    app.nostr_sync()
        .publish(&request.category, &request.payload)
        .await
}

#[tauri::command]
pub async fn fetch<R: Runtime>(
    app: AppHandle<R>,
    request: FetchRequest,
) -> Result<Option<FetchResult>> {
    app.nostr_sync().fetch(&request.category).await
}

#[tauri::command]
pub async fn sync_all<R: Runtime>(
    app: AppHandle<R>,
    request: SyncAllRequest,
) -> Result<Vec<FetchResult>> {
    app.nostr_sync().sync_all(&request.categories).await
}

#[tauri::command]
pub async fn add_relay<R: Runtime>(app: AppHandle<R>, url: String) -> Result<()> {
    app.nostr_sync().add_relay(&url).await
}

#[tauri::command]
pub async fn remove_relay<R: Runtime>(app: AppHandle<R>, url: String) -> Result<()> {
    app.nostr_sync().remove_relay(&url).await
}

#[tauri::command]
pub async fn get_relays<R: Runtime>(app: AppHandle<R>) -> Result<Vec<RelayInfo>> {
    Ok(app.nostr_sync().relays().await)
}

#[tauri::command]
pub async fn get_pubkey<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>> {
    Ok(app.nostr_sync().pubkey().await.map(|pk| pk.to_hex()))
}

#[tauri::command]
pub async fn get_status<R: Runtime>(app: AppHandle<R>) -> Result<SyncStatus> {
    Ok(app.nostr_sync().status().await)
}

#[tauri::command]
pub async fn poll<R: Runtime>(app: AppHandle<R>, request: PollRequest) -> Result<Vec<FetchResult>> {
    app.nostr_sync().poll(&request.categories).await
}
