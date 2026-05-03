use serde::{ser::Serializer, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("nostr error: {0}")]
    Nostr(#[from] nostr_sdk::client::Error),

    #[error("signer not set — call set_signer before publishing or fetching")]
    SignerNotSet,

    #[error("payload too large: {size} bytes exceeds {limit} byte limit")]
    PayloadTooLarge { size: usize, limit: usize },

    #[error("encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("invalid namespace '{0}': must be non-empty and contain no '/' characters")]
    InvalidNamespace(String),

    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
