use crate::{Error, Result};

// NostrSyncState will be implemented in Task 6.
pub struct NostrSyncState;

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
}
