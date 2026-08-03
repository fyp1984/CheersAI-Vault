use crate::error::FileBayError;

/// The only remote path prefix this crate will ever construct or accept.
/// Established by the existing desktop FileBay upload whitelist.
pub const REMOTE_PATH_PREFIX: &str = "masked/";

/// Validates a fully-formed remote path before it is ever sent to FileBay.
/// Rejects path traversal, backslashes, doubled slashes, and control/query/
/// fragment characters, and requires the fixed `masked/` prefix.
pub fn validate_remote_path(path: &str) -> Result<(), FileBayError> {
    if !path.starts_with(REMOTE_PATH_PREFIX) || path.len() <= REMOTE_PATH_PREFIX.len() {
        return Err(FileBayError::UploadDenied);
    }
    if path.contains("..") || path.contains('\\') || path.contains("//") {
        return Err(FileBayError::UploadDenied);
    }
    if path.chars().any(|c| c.is_control() || c == '?' || c == '#') {
        return Err(FileBayError::UploadDenied);
    }
    Ok(())
}

/// Strips everything except ASCII alphanumerics, `-` and `_` from a
/// caller-supplied display name, so it is safe to embed in a server-built
/// remote path. Never derived from raw client input for the path itself —
/// callers must still combine the result with a server-controlled artifact
/// id via [`build_remote_path`].
pub fn sanitize_stem(display_name: &str) -> String {
    let stem = display_name
        .rsplit_once('.')
        .map(|(stem, _ext)| stem)
        .unwrap_or(display_name);
    let cleaned: String = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(80)
        .collect();
    if cleaned.is_empty() {
        "artifact".to_string()
    } else {
        cleaned
    }
}

/// Deterministically builds a remote path from a server-controlled artifact
/// id and a sanitized stem. This is the *only* way a caller should produce
/// a remote path for an upload it did not receive verbatim from the client
/// UI's own confirmed-and-echoed value.
pub fn build_remote_path(artifact_id: &str, safe_stem: &str) -> String {
    format!("{REMOTE_PATH_PREFIX}{artifact_id}-{safe_stem}.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_masked_remote_paths_are_allowed() {
        assert!(validate_remote_path("masked/report.md").is_ok());
        assert!(validate_remote_path("raw/report.md").is_err());
        assert!(validate_remote_path("masked/../secret.md").is_err());
        assert!(validate_remote_path("masked\\report.md").is_err());
        assert!(validate_remote_path("masked//report.md").is_err());
        assert!(validate_remote_path("masked/rep?ort.md").is_err());
        assert!(validate_remote_path("masked/").is_err());
    }

    #[test]
    fn sanitize_stem_strips_unsafe_characters_and_extension() {
        assert_eq!(sanitize_stem("report v1 (final).md"), "reportv1final");
        assert_eq!(sanitize_stem("完全非ASCII.md"), "ASCII");
        assert_eq!(sanitize_stem("完全中文档案.md"), "artifact");
    }

    #[test]
    fn sanitize_stem_falls_back_to_artifact_for_traversal_only_input() {
        // No safe characters survive filtering once the (dot-heavy) input is
        // split on its last '.', so this safely degrades to the fixed
        // fallback rather than ever emitting a path-traversal-shaped stem.
        assert_eq!(sanitize_stem("../../etc/passwd"), "artifact");
    }

    #[test]
    fn build_remote_path_is_deterministic_and_masked_prefixed() {
        let path = build_remote_path("artifact-123", "report");
        assert_eq!(path, "masked/artifact-123-report.md");
        assert!(validate_remote_path(&path).is_ok());
    }
}
