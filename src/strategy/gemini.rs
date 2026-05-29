//! Gemini countTokens API. The input is wrapped as a single content part; the
//! returned `totalTokens` is the provider *count* over content + a small fixed
//! request envelope (recorded as-is).

use serde::Deserialize;

use crate::error::TokntError;
use crate::options::{CountOptions, NetworkPolicy};
use crate::result::{Accuracy, Basis, CountResult, Strategy};

const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CountTokensResponse {
    total_tokens: usize,
}

pub(crate) fn count(
    text: &str,
    model: &str,
    opts: &CountOptions,
) -> Result<CountResult, TokntError> {
    if opts.network == NetworkPolicy::Offline {
        return Err(TokntError::OfflineApiBlocked {
            model: model.to_string(),
            provider: "gemini",
        });
    }
    let key = opts
        .gemini_key
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("GEMINI_API_KEY")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .ok_or(TokntError::MissingApiKey {
            provider: "gemini",
            env_var: "GEMINI_API_KEY",
        })?;

    let url = format!("{API_BASE}/{model}:countTokens");
    let response = reqwest::blocking::Client::new()
        .post(url)
        .header("x-goog-api-key", key)
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "contents": [{ "parts": [{ "text": text }] }],
        }))
        .send()
        .map_err(|e| TokntError::ProviderApi {
            provider: "gemini",
            detail: e.to_string(),
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(TokntError::ProviderApi {
            provider: "gemini",
            detail: format!(
                "HTTP {}: {}",
                status.as_u16(),
                super::extract_api_message(&body)
            ),
        });
    }

    let parsed: CountTokensResponse = response.json().map_err(|e| TokntError::ProviderApi {
        provider: "gemini",
        detail: format!("could not parse response: {e}"),
    })?;

    Ok(CountResult {
        model: model.to_string(),
        resolved_model: model.to_string(),
        strategy: Strategy::Gemini,
        encoding: None,
        revision: None,
        add_special_tokens: None,
        basis: Basis::ContentEnvelope,
        accuracy: Accuracy::ProviderCount,
        approximation: None,
        tokens: parsed.total_tokens,
    })
}
