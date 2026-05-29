//! Open-weight HF tokenizers via `tokenizers` on a cached `tokenizer.json`.
//! Raw-content basis: `encode(text, add_special_tokens = false)` — no BOS/EOS.

use tokenizers::Tokenizer;

use crate::cache;
use crate::error::TokntError;
use crate::options::CountOptions;
use crate::result::{Accuracy, Basis, CountResult, Strategy};

pub(crate) fn count(
    text: &str,
    repo: &str,
    opts: &CountOptions,
) -> Result<CountResult, TokntError> {
    let (path, revision) = cache::ensure_tokenizer(repo, opts)?;
    let tokenizer = Tokenizer::from_file(&path).map_err(|e| TokntError::TokenizerLoad {
        model: repo.to_string(),
        detail: e.to_string(),
    })?;
    let encoding = tokenizer
        .encode(text, false)
        .map_err(|e| TokntError::TokenizerLoad {
            model: repo.to_string(),
            detail: e.to_string(),
        })?;
    Ok(CountResult {
        model: repo.to_string(),
        resolved_model: repo.to_string(),
        strategy: Strategy::OpenWeight,
        encoding: None,
        revision: Some(revision),
        add_special_tokens: Some(false),
        basis: Basis::RawContent,
        accuracy: Accuracy::Exact,
        approximation: None,
        tokens: encoding.get_ids().len(),
    })
}
