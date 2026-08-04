use crate::error::FileBayError;

/// Validates a Gitea/FileBay `owner` or `repo` identifier: non-empty,
/// bounded length, and restricted to characters Gitea itself accepts for
/// repository/user slugs. Used both to validate admin-provided environment
/// configuration and to reject any attempt (browser or otherwise) to smuggle
/// an unexpected identifier into a request.
pub fn validate_identity(value: &str) -> Result<(), FileBayError> {
    if value.is_empty() || value.len() > 100 {
        return Err(FileBayError::ConfigInvalid);
    }
    if value.starts_with('.') || value.starts_with('-') || value.ends_with('.') {
        return Err(FileBayError::ConfigInvalid);
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(FileBayError::ConfigInvalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typical_gitea_identifiers() {
        assert!(validate_identity("cheersai-vault").is_ok());
        assert!(validate_identity("acme_corp.internal").is_ok());
    }

    #[test]
    fn rejects_empty_too_long_or_leading_punctuation() {
        assert!(validate_identity("").is_err());
        assert!(validate_identity(&"a".repeat(101)).is_err());
        assert!(validate_identity(".hidden").is_err());
        assert!(validate_identity("-leading-dash").is_err());
        assert!(validate_identity("trailing-dot.").is_err());
    }

    #[test]
    fn rejects_path_traversal_and_control_characters() {
        assert!(validate_identity("../secret").is_err());
        assert!(validate_identity("owner/repo").is_err());
        assert!(validate_identity("owner name").is_err());
        assert!(validate_identity("owner\u{0000}").is_err());
    }
}
