//! The four counting strategies behind a common dispatch, plus the proxy
//! (`approx`) path. Each submodule produces a [`crate::CountResult`].

pub(crate) mod anthropic;
pub(crate) mod approx;
pub(crate) mod gemini;
pub(crate) mod openweight;
pub(crate) mod tiktoken;

/// Pull a human-readable message out of a provider error body. Anthropic and
/// Gemini both nest it at `error.message`; fall back to the raw body.
pub(super) fn extract_api_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.trim().to_string())
}
