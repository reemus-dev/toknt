//! The result of a count and the small, serializable vocabulary describing it.
//!
//! These types are the stable surface Goal 2's CLI renders; their serde shapes
//! also define the `verification.json` artifact, so the field names/orders here
//! are intentional.

use serde::Serialize;

/// Which counting mechanism produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    /// OpenAI BPE via `tiktoken-rs` (offline, exact).
    Tiktoken,
    /// Hugging Face `tokenizer.json` via `tokenizers` (offline after first fetch, exact).
    OpenWeight,
    /// Anthropic `count_tokens` endpoint (network, provider estimate).
    Anthropic,
    /// Gemini `countTokens` endpoint (network, provider count).
    Gemini,
    /// A deliberate proxy estimate (see [`Accuracy::Approximate`]); never exact.
    Approx,
}

/// What the count is measured over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Basis {
    /// The tokens of the input text itself — no chat template/role wrapping.
    #[serde(rename = "raw-content")]
    RawContent,
    /// Content plus the small, fixed per-request message envelope a provider
    /// count necessarily includes. Inherent to the API; recorded, never hidden.
    #[serde(rename = "content+envelope")]
    ContentEnvelope,
}

/// How faithful the count is to the model's own tokenizer on the stated basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Accuracy {
    /// Exact raw-content count from the model's real tokenizer.
    Exact,
    /// Authoritative provider count for the submitted payload.
    ProviderCount,
    /// Provider estimate (may include provider-added tokens).
    ProviderEstimate,
    /// A proxy-tokenizer estimate; never to be presented as exact.
    Approximate,
}

/// The tiktoken encoding used as a stand-in when an exact/provider count is
/// unavailable but the caller accepted an estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ProxyEncoding {
    #[serde(rename = "o200k_base")]
    O200kBase,
    #[serde(rename = "cl100k_base")]
    Cl100kBase,
}

/// Why a count fell back to an approximation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApproxReason {
    /// No model was supplied.
    NoModel,
    /// A required provider API key was absent.
    MissingApiKey,
    /// The model could not be mapped to a known strategy/encoding.
    UnsupportedModel,
    /// Offline policy blocked the only path that could count it exactly.
    Offline,
}

/// Metadata attached to an approximate count; `None` for exact/provider counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Approximation {
    pub proxy_encoding: ProxyEncoding,
    pub reason: ApproxReason,
}

/// The outcome of counting `input` for a model under a [`crate::CountOptions`].
///
/// Field order mirrors the `verification.json` example; strategy-specific
/// metadata (`encoding`, `revision`, `add_special_tokens`) is omitted from the
/// serialized form where it does not apply.
#[derive(Debug, Clone, Serialize)]
pub struct CountResult {
    /// The model id exactly as requested by the caller.
    pub model: String,
    /// The concrete target it resolved to (encoding name, HF repo, or model id).
    pub resolved_model: String,
    pub strategy: Strategy,
    /// tiktoken encoding name (tiktoken/approx strategies only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// Resolved Hugging Face revision/commit (open-weight only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Whether special tokens were added during encoding (open-weight only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_special_tokens: Option<bool>,
    pub basis: Basis,
    pub accuracy: Accuracy,
    /// Present only for approximate counts.
    pub approximation: Option<Approximation>,
    pub tokens: usize,
}
