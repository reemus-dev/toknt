//! The counting core's error type. Messages are specific and actionable so
//! Goal 2's CLI can surface them verbatim (which env var to set, which command
//! to run, which input was bad).

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TokntError {
    #[error(
        "unknown model {model:?}: prefix it with openai:/anthropic:/google:/hf:, \
         pass a bare Hugging Face id like \"org/name\", or a tiktoken encoding \
         (o200k_base, o200k_harmony, cl100k_base, p50k_base, p50k_edit, r50k_base, gpt2)."
    )]
    UnknownModel { model: String },

    #[error(
        "unsupported OpenAI model {model:?}: it is not in the known tiktoken model map. \
         Pass an explicit encoding (e.g. o200k_base or cl100k_base) or check the model \
         name against the current OpenAI/tiktoken docs."
    )]
    UnsupportedOpenAiModel { model: String },

    #[error(
        "unknown tiktoken encoding {encoding:?}: valid encodings are o200k_harmony, \
         o200k_base, cl100k_base, p50k_base, p50k_edit, r50k_base, gpt2."
    )]
    UnknownEncoding { encoding: String },

    #[error("{provider} requires the {env_var} environment variable to be set.")]
    MissingApiKey {
        provider: &'static str,
        env_var: &'static str,
    },

    #[error(
        "offline mode: cannot count API-backed model {model:?} ({provider}) — it requires \
         a network request. Re-run with network access allowed."
    )]
    OfflineApiBlocked {
        model: String,
        provider: &'static str,
    },

    #[error(
        "offline mode: the tokenizer for {model:?} is not in the toknt cache. Fetch it \
         first (e.g. `toknt pull {model}`) or allow network access."
    )]
    OfflineCacheMiss { model: String },

    #[error("could not fetch the tokenizer for {model:?} from the Hugging Face Hub: {detail}")]
    HfDownload { model: String, detail: String },

    #[error(
        "access to the gated Hugging Face model {model:?} was denied. Set HF_TOKEN to a \
         token with access and accept the model license at https://huggingface.co/{model}."
    )]
    HfGated { model: String },

    #[error("failed to load the tokenizer file for {model:?}: {detail}")]
    TokenizerLoad { model: String, detail: String },

    #[error("the {provider} API request failed: {detail}")]
    ProviderApi {
        provider: &'static str,
        detail: String,
    },

    #[error(
        "{what} is not valid UTF-8 (first invalid byte at offset {offset}); toknt counts \
         UTF-8 text and will not lossy-decode."
    )]
    InvalidUtf8 { what: String, offset: usize },

    #[error("could not resolve the toknt cache directory: {detail}")]
    CacheDir { detail: String },
}

impl TokntError {
    /// True for failures where an `--approx` proxy estimate is a reasonable
    /// fallback (no usable exact/provider path), as opposed to hard input or
    /// transport errors that must surface.
    pub(crate) fn approx_eligible(&self) -> bool {
        matches!(
            self,
            TokntError::UnknownModel { .. }
                | TokntError::UnsupportedOpenAiModel { .. }
                | TokntError::UnknownEncoding { .. }
                | TokntError::MissingApiKey { .. }
                | TokntError::OfflineApiBlocked { .. }
                | TokntError::OfflineCacheMiss { .. }
        )
    }
}
