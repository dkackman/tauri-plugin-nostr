use std::sync::Arc;

use tauri::{AppHandle, Runtime};

use crate::{FetchResult, NostrSyncState, RelayInfo, Result, SyncStatus};

pub fn init<R: Runtime>(app: &AppHandle<R>) -> crate::Result<TauriPluginNostrSync<R>> {
    let state = Arc::new(NostrSyncState::new("default")?);
    Ok(TauriPluginNostrSync {
        _app: app.clone(),
        pub_state: state,
    })
}

pub struct TauriPluginNostrSync<R: Runtime> {
    _app: AppHandle<R>,
    pub(crate) pub_state: Arc<NostrSyncState>,
}

impl<R: Runtime> TauriPluginNostrSync<R> {
    pub async fn set_signer(
        &self,
        signer: impl nostr_sdk::NostrSigner + 'static,
    ) -> Result<()> {
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

    pub async fn publish(&self, category: &str, payload: &serde_json::Value) -> Result<()> {
        self.pub_state.publish(category, payload).await
    }

    pub async fn fetch(&self, category: &str) -> Result<Option<FetchResult>> {
        self.pub_state.fetch(category).await
    }
}
