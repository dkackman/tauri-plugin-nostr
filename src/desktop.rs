use std::sync::Arc;

use tauri::{AppHandle, Emitter, Runtime};

use crate::{FetchResult, NostrSyncState, RelayInfo, Result, SyncStatus};

pub fn init<R: Runtime>(
    app: &AppHandle<R>,
    relays: Vec<String>,
    namespace: &str,
    device_id: &str,
    max_payload_size: usize,
) -> crate::Result<TauriPluginNostrSync<R>> {
    let state = Arc::new(NostrSyncState::new(namespace, device_id, max_payload_size)?);
    let plugin = TauriPluginNostrSync {
        app: app.clone(),
        pub_state: state,
    };
    // Relay connections are kicked off async; errors surface later via status().
    let state_clone = plugin.pub_state.clone();
    tauri::async_runtime::spawn(async move {
        for url in relays {
            let _ = state_clone.add_relay(&url).await;
        }
    });
    Ok(plugin)
}

pub struct TauriPluginNostrSync<R: Runtime> {
    app: AppHandle<R>,
    pub(crate) pub_state: Arc<NostrSyncState>,
}

impl<R: Runtime> TauriPluginNostrSync<R> {
    pub async fn set_signer(&self, signer: impl nostr_sdk::NostrSigner + 'static) -> Result<()> {
        self.pub_state.set_signer(signer).await
    }

    pub async fn clear_signer(&self) {
        self.pub_state.clear_signer().await
    }

    pub async fn status(&self) -> SyncStatus {
        self.pub_state.status().await
    }

    pub async fn pubkey(&self) -> Option<nostr_sdk::PublicKey> {
        self.pub_state.pubkey().await
    }

    pub async fn add_relay(&self, url: &str) -> Result<()> {
        self.pub_state.add_relay(url).await
    }

    pub async fn remove_relay(&self, url: &str) -> Result<()> {
        self.pub_state.remove_relay(url).await
    }

    pub async fn relays(&self) -> Vec<RelayInfo> {
        self.pub_state.relays().await
    }

    pub async fn publish(
        &self,
        category: &str,
        payload: &serde_json::Value,
        expiration: Option<u64>,
    ) -> Result<()> {
        self.pub_state.publish(category, payload, expiration).await
    }

    pub async fn fetch(&self, category: &str) -> Result<Option<FetchResult>> {
        self.pub_state.fetch(category).await
    }

    pub async fn sync_all(&self, categories: &[String]) -> Result<Vec<FetchResult>> {
        self.pub_state.sync_all(categories).await
    }

    pub async fn wait_for_connection(&self, timeout: std::time::Duration) {
        self.pub_state.wait_for_connection(timeout).await
    }

    pub async fn poll(&self, categories: &[String]) -> Result<Vec<FetchResult>> {
        let updates = self.pub_state.poll(categories).await?;
        for update in &updates {
            let _ = self.app.emit("nostr-sync://updated", update);
        }
        Ok(updates)
    }
}
