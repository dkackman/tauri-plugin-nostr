use std::sync::Arc;
use std::time::Duration;

use nostr_sdk::{
    Client, EventBuilder, Filter, Kind, NostrSigner, Options, PublicKey, RelayStatus, Tag,
};

use crate::{Error, RelayInfo, Result, SyncStatus};

pub struct NostrSyncState {
    pub(crate) namespace: String,
    pub(crate) device_id: String,
    pub(crate) client: Client,
}

impl NostrSyncState {
    pub fn new(namespace: &str) -> Result<Self> {
        validate_namespace(namespace)?;
        let opts = Options::default().autoconnect(true);
        let client = Client::builder().opts(opts).build();
        Ok(Self {
            namespace: namespace.to_string(),
            device_id: uuid::Uuid::new_v4().to_string(),
            client,
        })
    }

    pub async fn set_signer(&self, signer: impl NostrSigner + 'static) -> Result<()> {
        self.client.set_signer(signer).await;
        Ok(())
    }

    pub async fn clear_signer(&self) {
        self.client.unset_signer().await;
    }

    pub async fn pubkey(&self) -> Option<PublicKey> {
        let signer = self.client.signer().await.ok()?;
        signer.get_public_key().await.ok()
    }

    pub async fn status(&self) -> SyncStatus {
        let has_signer = self.client.has_signer().await;
        let relays_map = self.client.relays().await;
        let relay_count = relays_map.len();
        let connected_relay_count = relays_map
            .values()
            .filter(|r| matches!(r.status(), RelayStatus::Connected))
            .count();

        SyncStatus {
            ready: has_signer && connected_relay_count > 0,
            relay_count,
            connected_relay_count,
        }
    }

    pub async fn add_relay(&self, url: &str) -> Result<()> {
        self.client.add_relay(url).await?;
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

    pub async fn publish(&self, category: &str, payload: &serde_json::Value) -> Result<()> {
        let signer = self
            .client
            .signer()
            .await
            .map_err(|_| Error::SignerNotSet)?;

        let ciphertext = encrypt_payload(&signer, payload).await?;
        let dtag = build_dtag(&self.namespace, category);
        let kind = Kind::from(30078u16);

        let pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| Error::EncryptionFailed(e.to_string()))?;

        let device_tag = Tag::parse(vec!["device_id", &self.device_id])
            .expect("device_id tag construction is infallible");

        let unsigned = EventBuilder::new(kind, ciphertext)
            .tag(Tag::identifier(&dtag))
            .tag(device_tag)
            .build(pubkey);

        let event = signer
            .sign_event(unsigned)
            .await
            .map_err(|e| Error::SigningFailed(e.to_string()))?;

        self.client.send_event(event).await?;
        Ok(())
    }

    pub async fn fetch(&self, category: &str) -> Result<Option<crate::FetchResult>> {
        let signer = self
            .client
            .signer()
            .await
            .map_err(|_| Error::SignerNotSet)?;

        let pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| Error::DecryptionFailed(e.to_string()))?;

        let dtag = build_dtag(&self.namespace, category);
        let filter = Filter::new()
            .kind(Kind::from(30078u16))
            .author(pubkey)
            .identifier(dtag);

        let events = self
            .client
            .fetch_events(vec![filter], Duration::from_secs(10))
            .await?;

        let event = match events.first() {
            Some(e) => e.clone(),
            None => return Ok(None),
        };

        let payload = decrypt_payload(&signer, &event.content).await?;

        let device_id = event
            .tags
            .iter()
            .find_map(|t| {
                let slice = t.as_slice();
                if slice.first().map(|s| s.as_str()) == Some("device_id") {
                    slice.get(1).cloned()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| pubkey.to_hex());

        let updated_at = chrono::DateTime::from_timestamp(event.created_at.as_u64() as i64, 0)
            .unwrap_or_else(chrono::Utc::now);

        Ok(Some(crate::FetchResult {
            payload,
            updated_at,
            device_id,
        }))
    }
}

const PAYLOAD_LIMIT: usize = 64 * 1024; // 64KB

fn check_payload_size(json: &str) -> Result<()> {
    let size = json.len();
    if size > PAYLOAD_LIMIT {
        return Err(Error::PayloadTooLarge {
            size,
            limit: PAYLOAD_LIMIT,
        });
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
    let json =
        serde_json::to_string(payload).map_err(|e| Error::EncryptionFailed(e.to_string()))?;
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

    #[test]
    fn new_rejects_invalid_namespace() {
        assert!(matches!(
            NostrSyncState::new(""),
            Err(Error::InvalidNamespace(_))
        ));
        assert!(matches!(
            NostrSyncState::new("a/b"),
            Err(Error::InvalidNamespace(_))
        ));
    }

    #[tokio::test]
    async fn publish_without_signer_returns_signer_not_set() {
        let state = NostrSyncState::new("testapp").unwrap();
        let result = state
            .publish("ui-settings", &serde_json::json!({ "theme": "dark" }))
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
    async fn pubkey_is_none_without_signer() {
        let state = NostrSyncState::new("testapp").unwrap();
        assert!(state.pubkey().await.is_none());
    }

    #[tokio::test]
    async fn signer_lifecycle_exposes_then_hides_pubkey() {
        let state = NostrSyncState::new("testapp").unwrap();
        let keys = nostr_sdk::Keys::generate();
        let expected = keys.public_key();

        state.set_signer(keys).await.unwrap();
        assert_eq!(state.pubkey().await, Some(expected));

        state.clear_signer().await;
        assert!(state.pubkey().await.is_none());
    }
}
