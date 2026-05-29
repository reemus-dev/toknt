//! The proxy-estimate path. Reuses a tiktoken encoding to produce a count when
//! no exact/provider path is available and the caller accepted an estimate.
//! Always labeled [`Accuracy::Approximate`] — never reuses `exact`.

use super::tiktoken;
use crate::model::Encoding;
use crate::result::{
    Accuracy, ApproxReason, Approximation, Basis, CountResult, ProxyEncoding, Strategy,
};

pub(crate) fn count(
    text: &str,
    model: &str,
    proxy: ProxyEncoding,
    reason: ApproxReason,
) -> CountResult {
    let encoding = match proxy {
        ProxyEncoding::O200kBase => Encoding::O200kBase,
        ProxyEncoding::Cl100kBase => Encoding::Cl100kBase,
    };
    CountResult {
        model: model.to_string(),
        resolved_model: encoding.name().to_string(),
        strategy: Strategy::Approx,
        encoding: Some(encoding.name().to_string()),
        revision: None,
        add_special_tokens: None,
        basis: Basis::RawContent,
        accuracy: Accuracy::Approximate,
        approximation: Some(Approximation {
            proxy_encoding: proxy,
            reason,
        }),
        tokens: tiktoken::raw_count(text, encoding),
    }
}
