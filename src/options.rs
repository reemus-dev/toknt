//! Caller-facing policy for a count. Goal 2 maps CLI flags onto these; the
//! library never reads flags itself.

use std::path::PathBuf;

use crate::result::{ApproxReason, ProxyEncoding};

/// Whether the count may touch the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkPolicy {
    /// HTTP is permitted (HF fetch on cache miss; Anthropic/Gemini calls).
    #[default]
    AllowNetwork,
    /// A hard no-network contract: cached tokenizers and tiktoken still work;
    /// uncached HF and any API-backed model return an actionable error before
    /// a client is ever constructed.
    Offline,
}

/// Whether a proxy estimate is acceptable when an exact/provider count cannot be
/// produced. Goal 2's `--approx` maps here rather than approximating in the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApproxPolicy {
    /// Fail rather than estimate.
    #[default]
    ExactOnly,
    /// On a fallback-eligible failure, count with `proxy_encoding` and label the
    /// result approximate with `reason`.
    AllowApprox {
        proxy_encoding: ProxyEncoding,
        reason: ApproxReason,
    },
}

/// Options controlling a single [`crate::count`] call.
///
/// Credentials default to the environment (`HF_TOKEN`, `ANTHROPIC_API_KEY`,
/// `GEMINI_API_KEY`); the explicit fields let a caller override without mutating
/// process env.
#[derive(Debug, Clone, Default)]
pub struct CountOptions {
    pub network: NetworkPolicy,
    pub approx: ApproxPolicy,
    /// Override the tokenizer cache dir (else `TOKNT_CACHE_DIR`, else XDG cache).
    pub cache_dir: Option<PathBuf>,
    pub hf_token: Option<String>,
    pub anthropic_key: Option<String>,
    pub gemini_key: Option<String>,
}

impl CountOptions {
    /// Default options with a hard no-network policy.
    pub fn offline() -> Self {
        Self {
            network: NetworkPolicy::Offline,
            ..Self::default()
        }
    }
}
