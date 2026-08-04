/// A FileBay access token. Deliberately does **not** derive `Debug` or
/// `serde::Serialize` — the compiler will refuse any attempt to log,
/// `{:?}`-print, or accidentally serialize this value into a response,
/// error, or test snapshot.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}
