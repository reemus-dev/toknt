//! Model resolution: map a requested model id to a concrete [`Target`].
//!
//! Order (per the spec): explicit prefix → alias registry (OpenAI families +
//! provider prefixes) → bare HF id → raw tiktoken encoding name → error. The
//! registry is small data + exact prefix rules — never a blanket
//! `gpt-* -> o200k_base`, because older GPT families use other encodings.

use crate::error::TokntError;
use crate::result::Strategy;

/// A tiktoken encoding the OpenAI path can route to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    O200kHarmony,
    O200kBase,
    Cl100kBase,
    P50kBase,
    P50kEdit,
    R50kBase,
}

impl Encoding {
    pub fn name(self) -> &'static str {
        match self {
            Encoding::O200kHarmony => "o200k_harmony",
            Encoding::O200kBase => "o200k_base",
            Encoding::Cl100kBase => "cl100k_base",
            Encoding::P50kBase => "p50k_base",
            Encoding::P50kEdit => "p50k_edit",
            Encoding::R50kBase => "r50k_base",
        }
    }
}

/// The concrete counting target a model resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Tiktoken { encoding: Encoding },
    OpenWeight { repo: String },
    Anthropic { model: String },
    Gemini { model: String },
}

impl Target {
    pub fn strategy(&self) -> Strategy {
        match self {
            Target::Tiktoken { .. } => Strategy::Tiktoken,
            Target::OpenWeight { .. } => Strategy::OpenWeight,
            Target::Anthropic { .. } => Strategy::Anthropic,
            Target::Gemini { .. } => Strategy::Gemini,
        }
    }
}

/// Map a raw tiktoken encoding *name* to an [`Encoding`].
fn encoding_by_name(name: &str) -> Option<Encoding> {
    Some(match name {
        "o200k_harmony" => Encoding::O200kHarmony,
        "o200k_base" => Encoding::O200kBase,
        "cl100k_base" => Encoding::Cl100kBase,
        "p50k_base" => Encoding::P50kBase,
        "p50k_edit" => Encoding::P50kEdit,
        "r50k_base" | "gpt2" => Encoding::R50kBase,
        _ => return None,
    })
}

/// OpenAI *model*-name → encoding prefixes, mirroring tiktoken's
/// `MODEL_PREFIX_TO_ENCODING`. Ordered so the more specific o200k families
/// (`gpt-4o`, `gpt-4.1`, `gpt-4.5`) match before the legacy `gpt-4` / `gpt-3.5`
/// cl100k families. First prefix match wins. Shared by [`openai_model_encoding`]
/// and [`known_models`] so the resolver and the `models` listing never drift.
const OPENAI_PREFIXES: &[(&str, Encoding)] = &[
    ("gpt-oss", Encoding::O200kHarmony),
    ("o1", Encoding::O200kBase),
    ("o3", Encoding::O200kBase),
    ("o4-mini", Encoding::O200kBase),
    ("gpt-5", Encoding::O200kBase),
    ("gpt-4.1", Encoding::O200kBase),
    ("gpt-4.5", Encoding::O200kBase),
    ("gpt-4o", Encoding::O200kBase),
    ("chatgpt-4o", Encoding::O200kBase),
    ("gpt-4", Encoding::Cl100kBase),
    ("gpt-3.5-turbo", Encoding::Cl100kBase),
    ("gpt-35-turbo", Encoding::Cl100kBase),
    ("gpt-3.5", Encoding::Cl100kBase),
    ("text-embedding-ada-002", Encoding::Cl100kBase),
    ("text-embedding-3-", Encoding::Cl100kBase),
    ("text-davinci-002", Encoding::P50kBase),
    ("text-davinci-003", Encoding::P50kBase),
    ("code-davinci-", Encoding::P50kBase),
    ("davinci", Encoding::R50kBase),
    ("curie", Encoding::R50kBase),
    ("babbage", Encoding::R50kBase),
    ("ada", Encoding::R50kBase),
    ("gpt2", Encoding::R50kBase),
];

/// All tiktoken encoding names a user can pass directly as a model id.
const ENCODING_NAMES: &[&str] = &[
    "o200k_harmony",
    "o200k_base",
    "cl100k_base",
    "p50k_base",
    "p50k_edit",
    "r50k_base",
    "gpt2",
];

fn openai_model_encoding(model: &str) -> Option<Encoding> {
    OPENAI_PREFIXES
        .iter()
        .find(|(prefix, _)| model.starts_with(prefix))
        .map(|(_, enc)| *enc)
}

/// A human-facing entry in the known-model registry, for the `models`
/// subcommand. The list is data the resolver actually uses (OpenAI prefixes,
/// raw encodings) plus the open-ended provider/HF patterns it recognizes.
#[derive(Debug, Clone, Copy)]
pub struct ModelInfo {
    /// What a user types — an exact encoding name, a family prefix (`gpt-5`),
    /// or an open pattern (`claude*`, `org/name`).
    pub pattern: &'static str,
    /// The strategy this routes to.
    pub strategy: Strategy,
    /// What it resolves to or requires (e.g. `→ o200k_base`, `needs ANTHROPIC_API_KEY`).
    pub detail: &'static str,
}

/// The known-model registry as a flat, displayable list. Backs `toknt models`.
/// Open namespaces (HF repos, unrecognized `claude*`/`gemini*` ids) are shown as
/// patterns, not exhaustively — the resolver accepts any id matching them.
pub fn known_models() -> Vec<ModelInfo> {
    let mut models = Vec::new();

    // Raw tiktoken encodings (offline, exact) — type the name directly.
    for &name in ENCODING_NAMES {
        models.push(ModelInfo {
            pattern: name,
            strategy: Strategy::Tiktoken,
            detail: "tiktoken encoding · offline, exact",
        });
    }

    // OpenAI model families → encoding, from the same table the resolver uses.
    for &(prefix, enc) in OPENAI_PREFIXES {
        models.push(ModelInfo {
            pattern: prefix,
            strategy: Strategy::Tiktoken,
            detail: match enc {
                Encoding::O200kHarmony => "→ o200k_harmony · offline, exact",
                Encoding::O200kBase => "→ o200k_base · offline, exact",
                Encoding::Cl100kBase => "→ cl100k_base · offline, exact",
                Encoding::P50kBase => "→ p50k_base · offline, exact",
                Encoding::P50kEdit => "→ p50k_edit · offline, exact",
                Encoding::R50kBase => "→ r50k_base · offline, exact",
            },
        });
    }

    // Open-ended provider / HF namespaces the resolver recognizes by shape.
    models.push(ModelInfo {
        pattern: "claude*",
        strategy: Strategy::Anthropic,
        detail: "Anthropic count endpoint · provider estimate · needs ANTHROPIC_API_KEY",
    });
    models.push(ModelInfo {
        pattern: "gemini*",
        strategy: Strategy::Gemini,
        detail: "Gemini count endpoint · provider count · needs GEMINI_API_KEY",
    });
    models.push(ModelInfo {
        pattern: "org/name",
        strategy: Strategy::OpenWeight,
        detail: "Hugging Face tokenizer.json · offline after fetch · HF_TOKEN if gated",
    });

    models
}

/// Resolve the body after an explicit `openai:` prefix: a raw encoding name, or
/// a known OpenAI model alias. Unknown → a specific error (no silent proxy).
fn resolve_openai(rest: &str) -> Result<Target, TokntError> {
    if let Some(enc) = encoding_by_name(rest) {
        return Ok(Target::Tiktoken { encoding: enc });
    }
    if let Some(enc) = openai_model_encoding(rest) {
        return Ok(Target::Tiktoken { encoding: enc });
    }
    Err(TokntError::UnsupportedOpenAiModel {
        model: rest.to_string(),
    })
}

/// Resolve a model id to a [`Target`], or an actionable error.
pub fn resolve(model: &str) -> Result<Target, TokntError> {
    let model = model.trim();

    // 1. Explicit prefix forces the strategy.
    if let Some(rest) = model.strip_prefix("openai:") {
        return resolve_openai(rest);
    }
    if let Some(rest) = model.strip_prefix("anthropic:") {
        return Ok(Target::Anthropic {
            model: rest.to_string(),
        });
    }
    if let Some(rest) = model.strip_prefix("google:") {
        return Ok(Target::Gemini {
            model: rest.to_string(),
        });
    }
    if let Some(rest) = model.strip_prefix("hf:") {
        return Ok(Target::OpenWeight {
            repo: rest.to_string(),
        });
    }

    // 2. Alias registry — friendly provider model names (which never contain a
    //    '/'); a slashed name is always a bare HF id handled in step 3.
    if !model.contains('/') {
        if let Some(enc) = openai_model_encoding(model) {
            return Ok(Target::Tiktoken { encoding: enc });
        }
        if model.starts_with("claude") {
            return Ok(Target::Anthropic {
                model: model.to_string(),
            });
        }
        if model.starts_with("gemini") {
            return Ok(Target::Gemini {
                model: model.to_string(),
            });
        }
    }

    // 3. Bare HF id (anything with a '/').
    if model.contains('/') {
        return Ok(Target::OpenWeight {
            repo: model.to_string(),
        });
    }

    // 4. Raw tiktoken encoding name.
    if let Some(enc) = encoding_by_name(model) {
        return Ok(Target::Tiktoken { encoding: enc });
    }

    // 5. Unknown.
    Err(TokntError::UnknownModel {
        model: model.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(model: &str) -> Encoding {
        match resolve(model).unwrap() {
            Target::Tiktoken { encoding } => encoding,
            other => panic!("expected tiktoken for {model:?}, got {other:?}"),
        }
    }

    #[test]
    fn raw_encoding_routes_directly() {
        assert_eq!(enc("o200k_base"), Encoding::O200kBase);
        assert_eq!(enc("cl100k_base"), Encoding::Cl100kBase);
        assert_eq!(enc("o200k_harmony"), Encoding::O200kHarmony);
        assert_eq!(enc("gpt2"), Encoding::R50kBase);
    }

    #[test]
    fn modern_openai_aliases_route_to_o200k() {
        assert_eq!(enc("gpt-5"), Encoding::O200kBase);
        assert_eq!(enc("gpt-5.1"), Encoding::O200kBase);
        assert_eq!(enc("gpt-5.2"), Encoding::O200kBase);
        assert_eq!(enc("gpt-4o"), Encoding::O200kBase);
        assert_eq!(enc("gpt-4.1"), Encoding::O200kBase);
    }

    #[test]
    fn legacy_openai_aliases_route_to_cl100k_not_o200k() {
        assert_eq!(enc("gpt-4"), Encoding::Cl100kBase);
        assert_eq!(enc("gpt-4-turbo"), Encoding::Cl100kBase);
        assert_eq!(enc("gpt-3.5-turbo"), Encoding::Cl100kBase);
    }

    #[test]
    fn provider_aliases_route_to_api_strategies() {
        assert_eq!(
            resolve("claude-opus-4-8").unwrap(),
            Target::Anthropic {
                model: "claude-opus-4-8".into()
            }
        );
        assert_eq!(
            resolve("gemini-3.5-flash").unwrap(),
            Target::Gemini {
                model: "gemini-3.5-flash".into()
            }
        );
    }

    #[test]
    fn bare_hf_id_routes_to_open_weight() {
        assert_eq!(
            resolve("deepseek-ai/DeepSeek-V4-Pro").unwrap(),
            Target::OpenWeight {
                repo: "deepseek-ai/DeepSeek-V4-Pro".into()
            }
        );
    }

    #[test]
    fn explicit_prefixes_force_strategy() {
        assert_eq!(enc("openai:o200k_base"), Encoding::O200kBase);
        assert_eq!(enc("openai:gpt-4"), Encoding::Cl100kBase);
        assert_eq!(
            resolve("hf:org/model").unwrap(),
            Target::OpenWeight {
                repo: "org/model".into()
            }
        );
        assert_eq!(
            resolve("anthropic:claude-x").unwrap(),
            Target::Anthropic {
                model: "claude-x".into()
            }
        );
        assert_eq!(
            resolve("google:gemini-x").unwrap(),
            Target::Gemini {
                model: "gemini-x".into()
            }
        );
    }

    #[test]
    fn unknown_models_error() {
        assert!(matches!(
            resolve("totally-unknown-thing"),
            Err(TokntError::UnknownModel { .. })
        ));
        assert!(matches!(
            resolve("openai:not-a-real-model"),
            Err(TokntError::UnsupportedOpenAiModel { .. })
        ));
    }
}
