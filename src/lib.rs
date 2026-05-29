//! `toknt` counting core: an accurate, model-specific token count for arbitrary
//! UTF-8 input, against one of four provider strategies (tiktoken, open-weight
//! HF, Anthropic, Gemini), plus an explicit proxy-estimate path.
//!
//! The product is **accuracy**: offline tokenizer counts are exact-or-error;
//! API counts are labeled provider counts/estimates; approximation happens only
//! when the caller opts in and is always labeled — never silently exact.
//!
//! This crate is the reusable engine; Goal 2 builds the CLI on top of
//! [`count`], [`count_bytes`], the [`CountOptions`] policy surface, the
//! [`CountResult`] metadata, and the [`cache`] helpers — without reshaping it.

mod cache;
mod error;
mod model;
mod options;
mod result;
mod strategy;

pub use cache::{cached_path, list_cached, pull, remove_cached, CachedEntry};
pub use error::TokntError;
pub use model::{resolve, Encoding, Target};
pub use options::{ApproxPolicy, CountOptions, NetworkPolicy};
pub use result::{
    Accuracy, ApproxReason, Approximation, Basis, CountResult, ProxyEncoding, Strategy,
};

/// Convenience alias for the crate's fallible results.
pub type Result<T> = std::result::Result<T, TokntError>;

/// Count tokens in already-validated UTF-8 `input` for `model` under `options`.
///
/// On a fallback-eligible failure (unknown/unsupported model, missing API key,
/// offline-blocked path) the result depends on `options.approx`: with
/// [`ApproxPolicy::ExactOnly`] the error surfaces; with
/// [`ApproxPolicy::AllowApprox`] the count falls back to the proxy encoding and
/// is labeled [`Accuracy::Approximate`].
pub fn count(input: &str, model: &str, options: &CountOptions) -> Result<CountResult> {
    match model::resolve(model) {
        Ok(target) => match dispatch(input, model, target, options) {
            Ok(result) => Ok(result),
            Err(err) => maybe_approx(input, model, options, err),
        },
        Err(err) => maybe_approx(input, model, options, err),
    }
}

/// Validate `input` as UTF-8, then [`count`]. `what` labels the input source
/// (e.g. a path) for the [`TokntError::InvalidUtf8`] message.
pub fn count_bytes(
    input: &[u8],
    what: &str,
    model: &str,
    options: &CountOptions,
) -> Result<CountResult> {
    let text = std::str::from_utf8(input).map_err(|e| TokntError::InvalidUtf8 {
        what: what.to_string(),
        offset: e.valid_up_to(),
    })?;
    count(text, model, options)
}

fn dispatch(
    input: &str,
    model: &str,
    target: Target,
    options: &CountOptions,
) -> Result<CountResult> {
    match target {
        Target::Tiktoken { encoding } => Ok(strategy::tiktoken::count(input, model, encoding)),
        Target::OpenWeight { repo } => strategy::openweight::count(input, &repo, options),
        Target::Anthropic { model: target } => strategy::anthropic::count(input, &target, options),
        Target::Gemini { model: target } => strategy::gemini::count(input, &target, options),
    }
}

fn maybe_approx(
    input: &str,
    model: &str,
    options: &CountOptions,
    err: TokntError,
) -> Result<CountResult> {
    if let ApproxPolicy::AllowApprox {
        proxy_encoding,
        reason,
    } = options.approx
    {
        if err.approx_eligible() {
            return Ok(strategy::approx::count(
                input,
                model,
                proxy_encoding,
                reason,
            ));
        }
    }
    Err(err)
}
