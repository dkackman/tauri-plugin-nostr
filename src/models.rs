use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishRequest {
    pub category: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchRequest {
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    pub payload: Value,
    pub updated_at: DateTime<Utc>,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAllRequest {
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub ready: bool,
    pub relay_count: usize,
    pub connected_relay_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayInfo {
    pub url: String,
    pub connected: bool,
    pub last_seen: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn fetch_result_serializes_camel_case() {
        let result = FetchResult {
            payload: serde_json::json!({ "x": 1 }),
            updated_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            device_id: "abc".to_string(),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(
            json.get("updatedAt").is_some(),
            "expected camelCase updatedAt"
        );
        assert!(
            json.get("deviceId").is_some(),
            "expected camelCase deviceId"
        );
        assert!(json.get("updated_at").is_none());
        assert!(json.get("device_id").is_none());
    }

    #[test]
    fn sync_status_serializes_camel_case() {
        let status = SyncStatus {
            ready: false,
            relay_count: 2,
            connected_relay_count: 1,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert!(json.get("relayCount").is_some());
        assert!(json.get("connectedRelayCount").is_some());
        assert!(json.get("relay_count").is_none());
    }

    #[test]
    fn relay_info_serializes_camel_case() {
        let info = RelayInfo {
            url: "wss://relay.example".to_string(),
            connected: true,
            last_seen: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert!(json.get("lastSeen").is_some());
        assert!(json.get("last_seen").is_none());
    }

    #[test]
    fn sync_all_request_deserializes_categories() {
        let json = r#"{"categories":["ui-settings","wallet"]}"#;
        let req: SyncAllRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.categories, vec!["ui-settings", "wallet"]);
    }
}
