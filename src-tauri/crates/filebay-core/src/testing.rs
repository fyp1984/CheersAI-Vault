use crate::error::FileBayError;
use crate::token::Token;
use crate::transport::{Transport, TransportResponse};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// A scripted, in-memory [`Transport`] for tests. Never performs real
/// network I/O, never resolves DNS, never touches a socket. Records how
/// many calls were made so tests can assert "zero transport calls" for
/// cancel / negative / whitelist-rejection paths (C3/D2 in the adapter
/// task's acceptance criteria).
pub struct FakeTransport {
    responses: Mutex<Vec<(String, u16, Option<serde_json::Value>)>>,
    default_status: u16,
    call_count: AtomicUsize,
}

impl FakeTransport {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(Vec::new()),
            default_status: 200,
            call_count: AtomicUsize::new(0),
        }
    }

    pub fn with_default_status(status: u16) -> Self {
        Self {
            responses: Mutex::new(Vec::new()),
            default_status: status,
            call_count: AtomicUsize::new(0),
        }
    }

    /// Registers a scripted, one-shot response for the next request whose
    /// URL contains `url_contains`. Matches are consumed in insertion
    /// order per matching substring.
    pub fn stub(
        &self,
        url_contains: impl Into<String>,
        status: u16,
        json: Option<serde_json::Value>,
    ) {
        self.responses
            .lock()
            .unwrap()
            .push((url_contains.into(), status, json));
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    fn resolve(&self, url: &str) -> TransportResponse {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let mut responses = self.responses.lock().unwrap();
        if let Some(index) = responses
            .iter()
            .position(|(needle, _, _)| url.contains(needle.as_str()))
        {
            let (_, status, json) = responses.remove(index);
            TransportResponse { status, json }
        } else {
            TransportResponse {
                status: self.default_status,
                json: None,
            }
        }
    }
}

impl Default for FakeTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for FakeTransport {
    async fn get(
        &self,
        url: &str,
        _token: Option<&Token>,
    ) -> Result<TransportResponse, FileBayError> {
        Ok(self.resolve(url))
    }

    async fn post_json(
        &self,
        url: &str,
        _token: Option<&Token>,
        _body: serde_json::Value,
    ) -> Result<TransportResponse, FileBayError> {
        Ok(self.resolve(url))
    }

    async fn delete_json(
        &self,
        url: &str,
        _token: Option<&Token>,
        _body: serde_json::Value,
    ) -> Result<TransportResponse, FileBayError> {
        Ok(self.resolve(url))
    }
}
