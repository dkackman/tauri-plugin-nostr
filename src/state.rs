use std::sync::Arc;
use std::time::Duration;

use nostr_sdk::{
    Client, ClientOptions, EventBuilder, Filter, Kind, NostrSigner, PublicKey, RelayStatus, Tag,
    Timestamp,
};

use crate::{Error, RelayInfo, Result, SyncStatus};

pub const DEFAULT_PAYLOAD_LIMIT: usize = 64 * 1024;  // 64KB default
pub const MAX_PAYLOAD_LIMIT: usize = 400 * 1024;     // 400KB hard cap

pub struct NostrSyncState {
    pub(crate) namespace: String,
    pub(crate) device_id: String,
    pub(crate) client: Client,
    max_payload_size: usize,
    last_seen: tokio::sync::RwLock<std::collections::HashMap<String, Timestamp>>,
}

impl NostrSyncState {
    pub fn new(namespace: &str, device_id: &str, max_payload_size: usize) -> Result<Self> {
        validate_namespace(namespace)?;
        if max_payload_size > MAX_PAYLOAD_LIMIT {
            return Err(Error::InvalidPayloadLimit {
                requested: max_payload_size,
                max: MAX_PAYLOAD_LIMIT,
            });
        }
        let opts = ClientOptions::default().autoconnect(true);
        let client = Client::builder().opts(opts).build();
        Ok(Self {
            namespace: namespace.to_string(),
            device_id: device_id.to_string(),
            client,
            max_payload_size,
            last_seen: tokio::sync::RwLock::new(std::collections::HashMap::new()),
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
            device_id: self.device_id.clone(),
        }
    }

    pub async fn add_relay(&self, url: &str) -> Result<()> {
        self.client.add_relay(url).await?;
        Ok(())
    }

    /// Wait for at least one relay to reach the Connected state, up to `timeout`.
    /// Useful in tests and after `add_relay` when an immediate relay round-trip is needed.
    pub async fn wait_for_connection(&self, timeout: Duration) {
        self.client.wait_for_connection(timeout).await;
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
                last_seen: None, // TODO: populate from relay stats when nostr-sdk exposes last_event_at
            })
            .collect()
    }

    pub async fn publish(
        &self,
        category: &str,
        payload: &serde_json::Value,
        expiration: Option<u64>,
    ) -> Result<()> {
        validate_category(category)?;
        let signer = self
            .client
            .signer()
            .await
            .map_err(|_| Error::SignerNotSet)?;

        let ciphertext = encrypt_payload(&signer, payload, self.max_payload_size).await?;
        let dtag = build_dtag(&self.namespace, category);
        let kind = Kind::from(30078u16);

        let pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| Error::EncryptionFailed(e.to_string()))?;

        let device_tag = Tag::parse(vec!["device_id", &self.device_id])
            .expect("device_id tag construction is infallible");

        let mut builder = EventBuilder::new(kind, ciphertext)
            .tag(Tag::identifier(&dtag))
            .tag(device_tag);

        if let Some(ts) = expiration {
            builder = builder.tag(Tag::expiration(Timestamp::from(ts)));
        }

        let unsigned = builder.build(pubkey);

        let event = signer
            .sign_event(unsigned)
            .await
            .map_err(|e| Error::SigningFailed(e.to_string()))?;

        let output = self.client.send_event(&event).await?;
        if output.success.is_empty() {
            return Err(Error::NoRelaysAccepted);
        }
        Ok(())
    }

    pub async fn fetch(&self, category: &str) -> Result<Option<crate::FetchResult>> {
        validate_category(category)?;
        let signer = self
            .client
            .signer()
            .await
            .map_err(|_| Error::SignerNotSet)?;

        let pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| Error::DecryptionFailed(e.to_string()))?;

        let filter = build_filter(pubkey, &self.namespace, category);

        let events = self
            .client
            .fetch_events(filter, Duration::from_secs(10))
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

        let updated_at = chrono::DateTime::from_timestamp(event.created_at.as_secs() as i64, 0)
            .unwrap_or_else(chrono::Utc::now);

        Ok(Some(crate::FetchResult {
            category: category.to_string(),
            payload,
            updated_at,
            device_id,
        }))
    }

    pub async fn sync_all(&self, categories: &[String]) -> Result<Vec<crate::FetchResult>> {
        let mut results = Vec::new();
        for category in categories {
            if let Some(result) = self.fetch(category).await? {
                results.push(result);
            }
        }
        Ok(results)
    }

    pub async fn poll(&self, categories: &[String]) -> Result<Vec<crate::FetchResult>> {
        let signer = self
            .client
            .signer()
            .await
            .map_err(|_| Error::SignerNotSet)?;
        let pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| Error::DecryptionFailed(e.to_string()))?;

        let mut updates = Vec::new();

        for category in categories {
            validate_category(category)?;
            let filter = build_filter(pubkey, &self.namespace, category);
            let events = self
                .client
                .fetch_events(filter, Duration::from_secs(10))
                .await?;

            if let Some(event) = events.first() {
                let is_new = {
                    let seen = self.last_seen.read().await;
                    seen.get(category.as_str())
                        .map_or(true, |&ts| event.created_at > ts)
                };

                if is_new {
                    let payload = decrypt_payload(&signer, &event.content).await?;

                    self.last_seen
                        .write()
                        .await
                        .insert(category.clone(), event.created_at);

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

                    let updated_at =
                        chrono::DateTime::from_timestamp(event.created_at.as_secs() as i64, 0)
                            .unwrap_or_else(chrono::Utc::now);

                    updates.push(crate::FetchResult {
                        category: category.clone(),
                        payload,
                        updated_at,
                        device_id,
                    });
                }
            }
        }

        Ok(updates)
    }
}

fn check_payload_size(json: &str, limit: usize) -> Result<()> {
    let size = json.len();
    if size > limit {
        return Err(Error::PayloadTooLarge { size, limit });
    }
    Ok(())
}

async fn encrypt_payload(
    signer: &Arc<dyn NostrSigner>,
    payload: &serde_json::Value,
    limit: usize,
) -> Result<String> {
    let pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    let json =
        serde_json::to_string(payload).map_err(|e| Error::EncryptionFailed(e.to_string()))?;
    check_payload_size(&json, limit)?;
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

/// Constructs the NIP-78 d-tag value: `{namespace}/{category}/v1`
pub(crate) fn build_dtag(namespace: &str, category: &str) -> String {
    format!("{}/{}/v1", namespace, category)
}

fn build_filter(pubkey: PublicKey, namespace: &str, category: &str) -> Filter {
    Filter::new()
        .kind(Kind::from(30078u16))
        .author(pubkey)
        .identifier(build_dtag(namespace, category))
}

/// Validates that a namespace is non-empty and contains no '/' characters.
pub(crate) fn validate_namespace(namespace: &str) -> Result<()> {
    if namespace.is_empty() || namespace.contains('/') {
        return Err(Error::InvalidNamespace(namespace.to_string()));
    }
    Ok(())
}

/// Validates that a category is non-empty and contains no '/' characters.
/// A slash in the category would silently produce an extra d-tag path segment,
/// causing publish/fetch to operate on different identifiers.
fn validate_category(category: &str) -> Result<()> {
    if category.is_empty() || category.contains('/') {
        return Err(Error::InvalidCategory(category.to_string()));
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
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        let status = state.status().await;
        assert!(!status.ready);
    }

    #[tokio::test]
    async fn payload_encrypt_decrypt_roundtrip() {
        let keys = nostr_sdk::Keys::generate();
        let signer: Arc<dyn NostrSigner> = Arc::new(keys);
        let original = serde_json::json!({ "theme": "dark", "font_size": 14 });
        let encrypted = encrypt_payload(&signer, &original, DEFAULT_PAYLOAD_LIMIT).await.unwrap();
        let decrypted = decrypt_payload(&signer, &encrypted).await.unwrap();
        assert_eq!(original, decrypted);
    }

    #[test]
    fn payload_at_limit_is_accepted() {
        let json = "x".repeat(DEFAULT_PAYLOAD_LIMIT);
        assert!(check_payload_size(&json, DEFAULT_PAYLOAD_LIMIT).is_ok());
    }

    #[test]
    fn payload_over_limit_is_rejected() {
        let json = "x".repeat(DEFAULT_PAYLOAD_LIMIT + 1);
        assert!(matches!(
            check_payload_size(&json, DEFAULT_PAYLOAD_LIMIT),
            Err(Error::PayloadTooLarge { .. })
        ));
    }

    #[test]
    fn new_rejects_invalid_namespace() {
        assert!(matches!(
            NostrSyncState::new("", "test-device", DEFAULT_PAYLOAD_LIMIT),
            Err(Error::InvalidNamespace(_))
        ));
        assert!(matches!(
            NostrSyncState::new("a/b", "test-device", DEFAULT_PAYLOAD_LIMIT),
            Err(Error::InvalidNamespace(_))
        ));
    }

    #[test]
    fn category_rejects_empty_string() {
        assert!(matches!(
            validate_category(""),
            Err(Error::InvalidCategory(_))
        ));
    }

    #[test]
    fn category_rejects_slash() {
        assert!(matches!(
            validate_category("ui/settings"),
            Err(Error::InvalidCategory(_))
        ));
    }

    #[test]
    fn category_accepts_valid_identifier() {
        assert!(validate_category("ui-settings").is_ok());
        assert!(validate_category("wallet").is_ok());
    }

    #[tokio::test]
    async fn publish_with_slash_category_returns_invalid_category() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        state.set_signer(nostr_sdk::Keys::generate()).await.unwrap();
        let result = state
            .publish("ui/settings", &serde_json::json!({"x": 1}), None)
            .await;
        assert!(matches!(result, Err(Error::InvalidCategory(_))));
    }

    #[tokio::test]
    async fn fetch_with_slash_category_returns_invalid_category() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        state.set_signer(nostr_sdk::Keys::generate()).await.unwrap();
        let result = state.fetch("ui/settings").await;
        assert!(matches!(result, Err(Error::InvalidCategory(_))));
    }

    #[tokio::test]
    async fn clear_signer_prevents_publish() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        state.set_signer(nostr_sdk::Keys::generate()).await.unwrap();
        state.clear_signer().await;
        let result = state
            .publish("ui-settings", &serde_json::json!({"x": 1}), None)
            .await;
        assert!(matches!(result, Err(Error::SignerNotSet)));
    }

    #[tokio::test]
    async fn publish_without_signer_returns_signer_not_set() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        let result = state
            .publish("ui-settings", &serde_json::json!({ "theme": "dark" }), None)
            .await;
        assert!(matches!(result, Err(Error::SignerNotSet)));
    }

    #[tokio::test]
    async fn fetch_without_signer_returns_signer_not_set() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        let result = state.fetch("ui-settings").await;
        assert!(matches!(result, Err(Error::SignerNotSet)));
    }

    #[tokio::test]
    async fn pubkey_is_none_without_signer() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        assert!(state.pubkey().await.is_none());
    }

    #[tokio::test]
    async fn sync_all_without_signer_returns_signer_not_set() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        let categories = vec!["ui-settings".to_string(), "wallet".to_string()];
        let result = state.sync_all(&categories).await;
        assert!(matches!(result, Err(Error::SignerNotSet)));
    }

    #[tokio::test]
    async fn sync_all_with_empty_categories_returns_empty_vec() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        let keys = nostr_sdk::Keys::generate();
        state.set_signer(keys).await.unwrap();
        // No relay connected — sync_all with empty slice returns Ok([]) immediately
        let result = state.sync_all(&[]).await;
        assert!(matches!(result, Ok(ref v) if v.is_empty()));
    }

    #[tokio::test]
    async fn signer_lifecycle_exposes_then_hides_pubkey() {
        let state = NostrSyncState::new("testapp", "test-device", DEFAULT_PAYLOAD_LIMIT).unwrap();
        let keys = nostr_sdk::Keys::generate();
        let expected = keys.public_key();

        state.set_signer(keys).await.unwrap();
        assert_eq!(state.pubkey().await, Some(expected));

        state.clear_signer().await;
        assert!(state.pubkey().await.is_none());
    }

    #[test]
    fn new_rejects_payload_limit_over_max() {
        assert!(matches!(
            NostrSyncState::new("testapp", "test-device", MAX_PAYLOAD_LIMIT + 1),
            Err(Error::InvalidPayloadLimit { .. })
        ));
    }

    #[test]
    fn new_accepts_payload_limit_at_max() {
        assert!(NostrSyncState::new("testapp", "test-device", MAX_PAYLOAD_LIMIT).is_ok());
    }

    #[test]
    fn custom_limit_is_enforced() {
        let limit = 100;
        assert!(check_payload_size(&"x".repeat(limit), limit).is_ok());
        assert!(matches!(
            check_payload_size(&"x".repeat(limit + 1), limit),
            Err(Error::PayloadTooLarge { .. })
        ));
    }
}
