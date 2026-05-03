use std::collections::HashMap;
use std::sync::Arc;

use nostr_sdk::{Client, NostrSigner, PublicKey, RelayStatus, Timestamp};
use tokio::sync::RwLock;

use crate::{Error, RelayInfo, Result, SyncStatus};

pub struct NostrSyncState {
    pub(crate) namespace: String,
    pub(crate) device_id: String,
    pub(crate) client: Client,
    pub(crate) signer: RwLock<Option<Arc<dyn NostrSigner>>>,
    pub(crate) known_timestamps: RwLock<HashMap<String, Timestamp>>,
}

impl NostrSyncState {
    pub fn new(namespace: &str) -> Result<Self> {
        validate_namespace(namespace)?;
        Ok(Self {
            namespace: namespace.to_string(),
            device_id: uuid::Uuid::new_v4().to_string(),
            client: Client::default(),
            signer: RwLock::new(None),
            known_timestamps: RwLock::new(HashMap::new()),
        })
    }

    pub async fn set_signer(&self, signer: impl NostrSigner + Send + Sync + 'static) -> Result<()> {
        let mut guard = self.signer.write().await;
        *guard = Some(Arc::new(signer));
        Ok(())
    }

    pub async fn clear_signer(&self) {
        let mut guard = self.signer.write().await;
        *guard = None;
        // The Arc drops here; ZeroizeOnDrop on the underlying Keys zeroes key bytes.
    }

    pub async fn pubkey(&self) -> Option<PublicKey> {
        let guard = self.signer.read().await;
        let signer = guard.as_ref()?;
        signer.get_public_key().await.ok()
    }

    pub async fn status(&self) -> SyncStatus {
        let has_signer = self.signer.read().await.is_some();
        let relays_map = self.client.relays().await;
        let relay_count = relays_map.len();
        let connected_relay_count = relays_map
            .values()
            .filter(|r| matches!(r.status(), RelayStatus::Connected))
            .count();

        SyncStatus {
            ready: has_signer && connected_relay_count > 0,
            outbox_depth: 0,
            relay_count,
            connected_relay_count,
        }
    }

    pub async fn add_relay(&self, url: &str) -> Result<()> {
        self.client.add_relay(url).await?;
        self.client.connect_relay(url).await?;
        Ok(())
    }

    pub async fn remove_relay(&self, url: &str) -> Result<()> {
        self.client.remove_relay(url).await?;
        Ok(())
    }

    pub async fn relays(&self) -> Vec<RelayInfo> {
        let relays_map = self.client.relays().await;
        relays_map
            .into_iter()
            .map(|(url, relay)| RelayInfo {
                url: url.to_string(),
                connected: matches!(relay.status(), RelayStatus::Connected),
                last_seen: None,
            })
            .collect()
    }
}

const PAYLOAD_LIMIT: usize = 64 * 1024; // 64KB

fn check_payload_size(json: &str) -> Result<()> {
    let size = json.len();
    if size > PAYLOAD_LIMIT {
        return Err(Error::PayloadTooLarge { size, limit: PAYLOAD_LIMIT });
    }
    Ok(())
}

async fn encrypt_payload(
    signer: &Arc<dyn NostrSigner>,
    payload: &serde_json::Value,
) -> Result<String> {
    let pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    let json = serde_json::to_string(payload)
        .map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    check_payload_size(&json)?;
    signer
        .nip44_encrypt(&pubkey, &json)
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))
}

async fn decrypt_payload(
    signer: &Arc<dyn NostrSigner>,
    ciphertext: &str,
) -> Result<serde_json::Value> {
    let pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| Error::DecryptionFailed(e.to_string()))?;
    let json = signer
        .nip44_decrypt(&pubkey, ciphertext)
        .await
        .map_err(|e| Error::DecryptionFailed(e.to_string()))?;
    serde_json::from_str(&json).map_err(|e| Error::DecryptionFailed(e.to_string()))
}

/// Constructs the NIP-33 d-tag value: `{namespace}/{category}/v1`
pub(crate) fn build_dtag(namespace: &str, category: &str) -> String {
    format!("{}/{}/v1", namespace, category)
}

/// Validates that a namespace is non-empty and contains no '/' characters.
pub(crate) fn validate_namespace(namespace: &str) -> Result<()> {
    if namespace.is_empty() {
        return Err(Error::InvalidNamespace(namespace.to_string()));
    }
    if namespace.contains('/') {
        return Err(Error::InvalidNamespace(namespace.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtag_format_includes_namespace_category_and_version() {
        assert_eq!(build_dtag("sage", "ui-settings"), "sage/ui-settings/v1");
    }

    #[test]
    fn dtag_works_with_various_inputs() {
        assert_eq!(build_dtag("myapp", "wallet-config"), "myapp/wallet-config/v1");
    }

    #[test]
    fn namespace_rejects_empty_string() {
        assert!(matches!(
            validate_namespace(""),
            Err(Error::InvalidNamespace(_))
        ));
    }

    #[test]
    fn namespace_rejects_slash() {
        assert!(matches!(
            validate_namespace("bad/namespace"),
            Err(Error::InvalidNamespace(_))
        ));
    }

    #[test]
    fn namespace_accepts_valid_identifier() {
        assert!(validate_namespace("sage").is_ok());
        assert!(validate_namespace("my-app").is_ok());
        assert!(validate_namespace("app_v2").is_ok());
    }

    #[tokio::test]
    async fn sync_status_not_ready_without_signer() {
        let state = NostrSyncState::new("testapp").unwrap();
        let status = state.status().await;
        assert!(!status.ready);
        assert_eq!(status.outbox_depth, 0);
    }

    #[tokio::test]
    async fn payload_encrypt_decrypt_roundtrip() {
        let keys = nostr_sdk::Keys::generate();
        let signer: Arc<dyn NostrSigner> = Arc::new(keys);
        let original = serde_json::json!({ "theme": "dark", "font_size": 14 });
        let encrypted = encrypt_payload(&signer, &original).await.unwrap();
        let decrypted = decrypt_payload(&signer, &encrypted).await.unwrap();
        assert_eq!(original, decrypted);
    }

    #[test]
    fn payload_at_limit_is_accepted() {
        let json = "x".repeat(PAYLOAD_LIMIT);
        assert!(check_payload_size(&json).is_ok());
    }

    #[test]
    fn payload_over_limit_is_rejected() {
        let json = "x".repeat(PAYLOAD_LIMIT + 1);
        let result = check_payload_size(&json);
        assert!(matches!(result, Err(Error::PayloadTooLarge { .. })));
    }
}
