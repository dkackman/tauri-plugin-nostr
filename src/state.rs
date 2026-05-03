use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use nostr_sdk::{Client, EventBuilder, Filter, Kind, NostrSigner, PublicKey, RelayStatus, Tag, Timestamp};
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

    pub async fn set_signer(&self, signer: impl NostrSigner + 'static) -> Result<()> {
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

    pub async fn publish(&self, category: &str, payload: &serde_json::Value) -> Result<()> {
        let guard = self.signer.read().await;
        let signer = guard.as_ref().ok_or(Error::SignerNotSet)?;

        let ciphertext = encrypt_payload(signer, payload).await?;
        let dtag = build_dtag(&self.namespace, category);
        let kind = Kind::from(30078u16);

        let pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| Error::EncryptionFailed(e.to_string()))?;

        let device_tag = Tag::parse(vec!["device_id", &self.device_id])
            .unwrap_or_else(|_| Tag::identifier("unknown"));

        let unsigned = EventBuilder::new(kind, ciphertext)
            .tag(Tag::identifier(&dtag))
            .tag(device_tag)
            .build(pubkey);

        let event = signer
            .sign_event(unsigned)
            .await
            .map_err(|e| Error::EncryptionFailed(e.to_string()))?;

        self.client.send_event(event).await?;
        Ok(())
    }

    pub async fn fetch(&self, category: &str) -> Result<Option<crate::FetchResult>> {
        let guard = self.signer.read().await;
        let signer = guard.as_ref().ok_or(Error::SignerNotSet)?;

        let pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| Error::DecryptionFailed(e.to_string()))?;

        let dtag = build_dtag(&self.namespace, category);
        let filter = Filter::new()
            .kind(Kind::from(30078u16))
            .author(pubkey)
            .identifier(dtag.clone());

        let timeout = Duration::from_secs(10);
        let events = self.client.fetch_events(vec![filter], timeout).await?;

        // Events is already sorted descending by created_at; take the newest.
        let event = match events.first() {
            Some(e) => e.clone(),
            None => return Ok(None),
        };

        // Only surface the event if it's newer than what we've already seen.
        let mut timestamps = self.known_timestamps.write().await;
        let known = timestamps.get(category);
        if !is_newer(known, &event.created_at) {
            return Ok(None);
        }

        let payload = decrypt_payload(signer, &event.content).await?;

        // Extract device_id from event tags.
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

        timestamps.insert(category.to_string(), event.created_at);

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

/// Returns true if `incoming` is strictly newer than `known`, or if there is no known timestamp.
fn is_newer(known: Option<&Timestamp>, incoming: &Timestamp) -> bool {
    match known {
        None => true,
        Some(k) => incoming > k,
    }
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

    #[test]
    fn timestamp_newer_wins() {
        let old = Timestamp::from(100u64);
        let new = Timestamp::from(200u64);
        assert!(is_newer(Some(&old), &new));
    }

    #[test]
    fn timestamp_equal_is_discarded() {
        let t = Timestamp::from(100u64);
        assert!(!is_newer(Some(&t), &t));
    }

    #[test]
    fn timestamp_older_is_discarded() {
        let old = Timestamp::from(100u64);
        let new = Timestamp::from(200u64);
        assert!(!is_newer(Some(&new), &old));
    }

    #[test]
    fn timestamp_no_known_is_always_newer() {
        let t = Timestamp::from(100u64);
        assert!(is_newer(None, &t));
    }
}
