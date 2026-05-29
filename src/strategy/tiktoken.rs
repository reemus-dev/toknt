//! OpenAI BPE via `tiktoken-rs` — offline and exact. Rank tables are compiled
//! into the binary (intended; the "don't embed tokenizers" rule targets HF).

use tiktoken_rs::{
    cl100k_base_singleton, o200k_base_singleton, o200k_harmony_singleton, p50k_base_singleton,
    p50k_edit_singleton, r50k_base_singleton, CoreBPE,
};

use crate::model::Encoding;
use crate::result::{Accuracy, Basis, CountResult, Strategy};

fn bpe(encoding: Encoding) -> &'static CoreBPE {
    match encoding {
        Encoding::O200kHarmony => o200k_harmony_singleton(),
        Encoding::O200kBase => o200k_base_singleton(),
        Encoding::Cl100kBase => cl100k_base_singleton(),
        Encoding::P50kBase => p50k_base_singleton(),
        Encoding::P50kEdit => p50k_edit_singleton(),
        Encoding::R50kBase => r50k_base_singleton(),
    }
}

/// Raw-content token count via the ordinary path: special-token strings like
/// `<|endoftext|>` are counted as plain text and never raise.
pub(crate) fn raw_count(text: &str, encoding: Encoding) -> usize {
    bpe(encoding).encode_ordinary(text).len()
}

pub(crate) fn count(text: &str, model: &str, encoding: Encoding) -> CountResult {
    CountResult {
        model: model.to_string(),
        resolved_model: encoding.name().to_string(),
        strategy: Strategy::Tiktoken,
        encoding: Some(encoding.name().to_string()),
        revision: None,
        add_special_tokens: None,
        basis: Basis::RawContent,
        accuracy: Accuracy::Exact,
        approximation: None,
        tokens: raw_count(text, encoding),
    }
}
