use crate::error::FileBayError;

/// A validated FileBay HTTPS origin. Construction is the *only* place TLS
/// and URL shape rules are enforced — there is no way to obtain an
/// `Endpoint` that points at `http://`, carries userinfo/query/fragment, or
/// a non-root path, and no option anywhere in this crate to skip
/// certificate verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    origin: String,
}

impl Endpoint {
    pub fn parse(raw: &str) -> Result<Self, FileBayError> {
        let url = reqwest::Url::parse(raw).map_err(|_| FileBayError::ConfigInvalid)?;
        if url.scheme() != "https" {
            return Err(FileBayError::ConfigInvalid);
        }
        let Some(host) = url.host_str() else {
            return Err(FileBayError::ConfigInvalid);
        };
        if !url.username().is_empty() || url.password().is_some() {
            return Err(FileBayError::ConfigInvalid);
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(FileBayError::ConfigInvalid);
        }
        let path = url.path();
        if !(path.is_empty() || path == "/") {
            return Err(FileBayError::ConfigInvalid);
        }
        let mut origin = format!("https://{host}");
        if let Some(port) = url.port() {
            origin.push_str(&format!(":{port}"));
        }
        Ok(Self { origin })
    }

    pub fn as_str(&self) -> &str {
        &self.origin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_https_root_origin() {
        let endpoint = Endpoint::parse("https://filebay.example.com").unwrap();
        assert_eq!(endpoint.as_str(), "https://filebay.example.com");
    }

    #[test]
    fn accepts_https_origin_with_explicit_port_and_trailing_slash() {
        let endpoint = Endpoint::parse("https://filebay.example.com:8443/").unwrap();
        assert_eq!(endpoint.as_str(), "https://filebay.example.com:8443");
    }

    #[test]
    fn rejects_non_https_schemes() {
        assert_eq!(
            Endpoint::parse("http://filebay.example.com").unwrap_err(),
            FileBayError::ConfigInvalid
        );
        assert_eq!(
            Endpoint::parse("ftp://filebay.example.com").unwrap_err(),
            FileBayError::ConfigInvalid
        );
    }

    #[test]
    fn rejects_userinfo_query_fragment_and_non_root_path() {
        assert!(Endpoint::parse("https://user:pass@filebay.example.com").is_err());
        assert!(Endpoint::parse("https://filebay.example.com?x=1").is_err());
        assert!(Endpoint::parse("https://filebay.example.com#frag").is_err());
        assert!(Endpoint::parse("https://filebay.example.com/some/path").is_err());
    }

    #[test]
    fn rejects_malformed_or_hostless_input() {
        assert!(Endpoint::parse("not a url").is_err());
        assert!(Endpoint::parse("https://").is_err());
    }
}
