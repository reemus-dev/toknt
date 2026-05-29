//! Anthropic Count Tokens API. The input is wrapped as a single `user` message;
//! the returned `input_tokens` is a provider *estimate* over content + a small
//! fixed message envelope (recorded as-is, never subtracted).

use serde::Deserialize;

use crate::error::TokntError;
use crate::options::{CountOptions, NetworkPolicy};
use crate::result::{Accuracy, Basis, CountResult, Strategy};

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages/count_tokens";
const API_VERSION: &str = "2023-06-01";

#[derive(Deserialize)]
struct CountTokensResponse {
    input_tokens: usize,
}

pub(crate) fn count(
    text: &str,
    model: &str,
    opts: &CountOptions,
) -> Result<CountResult, TokntError> {
    if opts.network == NetworkPolicy::Offline {
        return Err(TokntError::OfflineApiBlocked {
            model: model.to_string(),
            provider: "anthropic",
        });
    }
    let key = opts
        .anthropic_key
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .ok_or(TokntError::MissingApiKey {
            provider: "claude",
            env_var: "ANTHROPIC_API_KEY",
        })?;

    let response = reqwest::blocking::Client::new()
        .post(ENDPOINT)
        .header("x-api-key", key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": text }],
        }))
        .send()
        .map_err(|e| TokntError::ProviderApi {
            provider: "anthropic",
            detail: e.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(TokntError::ProviderApi {
            provider: "anthropic",
            detail: format!(
                "HTTP {}: {}",
                status.as_u16(),
                super::extract_api_message(&body)
            ),
        });
    }

    let parsed: CountTokensResponse = response.json().map_err(|e| TokntError::ProviderApi {
        provider: "anthropic",
        detail: format!("could not parse response: {e}"),
    })?;

    Ok(CountResult {
        model: model.to_string(),
        resolved_model: model.to_string(),
        strategy: Strategy::Anthropic,
        encoding: None,
        revision: None,
        add_special_tokens: None,
        basis: Basis::ContentEnvelope,
        accuracy: Accuracy::ProviderEstimate,
        approximation: None,
        tokens: parsed.input_tokens,
    })
}
