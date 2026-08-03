use crate::error::FileBayError;
use crate::token::Token;

/// A minimal, transport-agnostic HTTP response: just enough for this
/// crate's Gitea-shaped API calls. Never exposes the underlying transport
/// error or raw response body to callers outside this crate.
pub struct TransportResponse {
    pub status: u16,
    pub json: Option<serde_json::Value>,
}

/// Injectable HTTP transport. Production code uses [`ReqwestTransport`];
/// Runtime/desktop tests inject a fake implementation so automated tests
/// never touch the network or a real FileBay instance. Implementations are
/// used only via a generic type parameter (never `dyn Transport`), so a
/// plain `async fn` here needs no boxing or extra crates.
pub trait Transport: Send + Sync {
    fn get(
        &self,
        url: &str,
        token: Option<&Token>,
    ) -> impl std::future::Future<Output = Result<TransportResponse, FileBayError>> + Send;

    fn post_json(
        &self,
        url: &str,
        token: Option<&Token>,
        body: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<TransportResponse, FileBayError>> + Send;

    fn delete_json(
        &self,
        url: &str,
        token: Option<&Token>,
        body: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<TransportResponse, FileBayError>> + Send;
}

/// Lets a host share one `Arc<SomeTransport>` between its own state and a
/// [`crate::FileBayClient`] instance — e.g. a test keeps an `Arc` clone to
/// call `FakeTransport::call_count()` after driving requests through a
/// client built from another clone of the same `Arc`. Blanket-forwards to
/// the inner transport; no behavior of its own.
impl<T: Transport> Transport for std::sync::Arc<T> {
    async fn get(
        &self,
        url: &str,
        token: Option<&Token>,
    ) -> Result<TransportResponse, FileBayError> {
        T::get(self, url, token).await
    }

    async fn post_json(
        &self,
        url: &str,
        token: Option<&Token>,
        body: serde_json::Value,
    ) -> Result<TransportResponse, FileBayError> {
        T::post_json(self, url, token, body).await
    }

    async fn delete_json(
        &self,
        url: &str,
        token: Option<&Token>,
        body: serde_json::Value,
    ) -> Result<TransportResponse, FileBayError> {
        T::delete_json(self, url, token, body).await
    }
}

/// Production transport. Built with `rustls-tls` only (see `Cargo.toml`);
/// nothing in this type accepts an option to skip certificate verification.
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestTransport {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    fn auth_header(token: Option<&Token>) -> Option<String> {
        token.map(|token| format!("token {}", token.as_str()))
    }

    async fn to_transport_response(
        response: reqwest::Response,
    ) -> Result<TransportResponse, FileBayError> {
        let status = response.status().as_u16();
        // Body is only parsed as JSON best-effort; a non-JSON or empty body
        // (common on 404/204) is not itself an error at this layer.
        let json = response.json::<serde_json::Value>().await.ok();
        Ok(TransportResponse { status, json })
    }
}

impl Transport for ReqwestTransport {
    async fn get(
        &self,
        url: &str,
        token: Option<&Token>,
    ) -> Result<TransportResponse, FileBayError> {
        let mut request = self.client.get(url);
        if let Some(header) = Self::auth_header(token) {
            request = request.header("Authorization", header);
        }
        let response = request
            .send()
            .await
            .map_err(|_| FileBayError::ConnectionFailed)?;
        Self::to_transport_response(response).await
    }

    async fn post_json(
        &self,
        url: &str,
        token: Option<&Token>,
        body: serde_json::Value,
    ) -> Result<TransportResponse, FileBayError> {
        let mut request = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&body);
        if let Some(header) = Self::auth_header(token) {
            request = request.header("Authorization", header);
        }
        let response = request
            .send()
            .await
            .map_err(|_| FileBayError::ConnectionFailed)?;
        Self::to_transport_response(response).await
    }

    async fn delete_json(
        &self,
        url: &str,
        token: Option<&Token>,
        body: serde_json::Value,
    ) -> Result<TransportResponse, FileBayError> {
        let mut request = self
            .client
            .request(reqwest::Method::DELETE, url)
            .header("Content-Type", "application/json")
            .json(&body);
        if let Some(header) = Self::auth_header(token) {
            request = request.header("Authorization", header);
        }
        let response = request
            .send()
            .await
            .map_err(|_| FileBayError::ConnectionFailed)?;
        Self::to_transport_response(response).await
    }
}
