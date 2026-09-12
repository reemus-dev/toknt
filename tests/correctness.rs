//! Correctness guards for the counting core. Hermetic and offline: tiktoken is
//! compiled-in, the open-weight basis is proven against a committed fixture, and
//! the network/approx policies are exercised without any HTTP. The full
//! open-weight and API paths are proven live by `verification.json`.

use std::path::{Path, PathBuf};

use toknt::{
    count, count_bytes, Accuracy, ApproxPolicy, ApproxReason, Basis, CountOptions, NetworkPolicy,
    ProxyEncoding, Strategy, TokntError,
};

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn fixture(rel: &str) -> PathBuf {
    Path::new(REPO_ROOT).join(rel)
}

fn read_sample(name: &str) -> Vec<u8> {
    std::fs::read(fixture(&format!("tests/fixtures/samples/{name}"))).expect("sample fixture")
}

// ---- tiktoken (offline, exact) -------------------------------------------

#[test]
fn empty_input_is_zero_for_offline_tokenizers() {
    for model in ["o200k_base", "cl100k_base", "gpt2"] {
        let result = count("", model, &CountOptions::default()).unwrap();
        assert_eq!(result.tokens, 0, "{model} should count empty input as 0");
        assert_eq!(result.accuracy, Accuracy::Exact);
        assert_eq!(result.basis, Basis::RawContent);
        assert_eq!(result.strategy, Strategy::Tiktoken);
    }
}

#[test]
fn non_empty_input_is_positive() {
    let bytes = read_sample("sample.txt");
    let result = count_bytes(&bytes, "sample.txt", "o200k_base", &CountOptions::default()).unwrap();
    assert!(result.tokens > 0);
    // The sample is a few hundred tokens; guard against an order-of-magnitude bug.
    assert!(
        (100..2000).contains(&result.tokens),
        "unexpected token count {}",
        result.tokens
    );
}

#[test]
fn special_token_strings_count_as_ordinary_text() {
    // The ordinary path must never collapse "<|endoftext|>" to a single special
    // token, and must not raise.
    let bytes = read_sample("special-tokens.txt");
    let result = count_bytes(
        &bytes,
        "special-tokens.txt",
        "o200k_base",
        &CountOptions::default(),
    )
    .unwrap();
    assert!(result.tokens > 0);

    // A lone special-token string encodes to multiple ordinary text tokens.
    let one = count("<|endoftext|>", "o200k_base", &CountOptions::default())
        .unwrap()
        .tokens;
    assert!(
        one > 1,
        "<|endoftext|> should be several text tokens, got {one}"
    );
}

#[test]
fn modern_and_legacy_openai_aliases_count_via_expected_encoding() {
    let opts = CountOptions::default();
    let text = "The quick brown fox jumps over the lazy dog.";

    // gpt-5.2 == o200k_base; both should agree token-for-token.
    let gpt5 = count(text, "gpt-5.2", &opts).unwrap();
    let o200k = count(text, "o200k_base", &opts).unwrap();
    assert_eq!(gpt5.tokens, o200k.tokens);
    assert_eq!(gpt5.resolved_model, "o200k_base");

    // gpt-4 / gpt-3.5-turbo == cl100k_base (not o200k_base).
    for legacy in ["gpt-4", "gpt-3.5-turbo"] {
        let r = count(text, legacy, &opts).unwrap();
        assert_eq!(r.resolved_model, "cl100k_base", "{legacy} -> cl100k_base");
        assert_eq!(r.encoding.as_deref(), Some("cl100k_base"));
    }
}

// ---- open-weight raw basis (committed fixture, no network) ----------------

#[test]
fn open_weight_raw_basis_omits_special_tokens() {
    use tokenizers::Tokenizer;

    let path = fixture("tests/fixtures/tokenizers/bert-base-uncased.tokenizer.json");
    let tok = Tokenizer::from_file(&path).expect("load bert fixture");

    // Empty input -> 0 even though add_special_tokens=true would add [CLS]/[SEP].
    assert_eq!(tok.encode("", false).unwrap().get_ids().len(), 0);

    let text = "tokenizers split text into subword units";
    // This is the exact call the open-weight strategy makes.
    let raw = tok.encode(text, false).unwrap().get_ids().len();
    let with_specials = tok.encode(text, true).unwrap().get_ids().len();
    assert!(raw > 0);
    // BERT wraps with exactly [CLS] … [SEP]; the raw basis must drop both.
    assert_eq!(
        with_specials,
        raw + 2,
        "add_special_tokens=false must omit the 2 BERT specials"
    );
}

// ---- network policy (offline is a hard no-network contract) ---------------

#[test]
fn offline_blocks_api_models_before_any_request() {
    let opts = CountOptions::offline();
    let err = count("hello", "claude-opus-4-8", &opts).unwrap_err();
    assert!(matches!(
        err,
        TokntError::OfflineApiBlocked {
            provider: "anthropic",
            ..
        }
    ));
    let err = count("hello", "gemini-3.5-flash", &opts).unwrap_err();
    assert!(matches!(
        err,
        TokntError::OfflineApiBlocked {
            provider: "gemini",
            ..
        }
    ));
}

#[test]
fn offline_uncached_open_weight_is_an_actionable_cache_miss() {
    let opts = CountOptions {
        network: NetworkPolicy::Offline,
        // Point at a guaranteed-empty cache dir so the lookup misses.
        cache_dir: Some(std::env::temp_dir().join("toknt-test-cache-miss-does-not-exist")),
        ..CountOptions::default()
    };
    let err = count("hello", "some-org/definitely-not-cached", &opts).unwrap_err();
    assert!(matches!(err, TokntError::OfflineCacheMiss { .. }));
}

#[test]
fn offline_policy_is_not_approximated() {
    let opts = CountOptions {
        network: NetworkPolicy::Offline,
        approx: ApproxPolicy::AllowApprox {
            proxy_encoding: ProxyEncoding::O200kBase,
            reason: ApproxReason::Offline,
        },
        cache_dir: Some(std::env::temp_dir().join("toknt-test-cache-miss-does-not-exist")),
        ..CountOptions::default()
    };

    let api_err = count("hello", "claude-opus-4-8", &opts).unwrap_err();
    assert!(matches!(api_err, TokntError::OfflineApiBlocked { .. }));

    let hf_err = count("hello", "some-org/definitely-not-cached", &opts).unwrap_err();
    assert!(matches!(hf_err, TokntError::OfflineCacheMiss { .. }));
}

// ---- approximation policy --------------------------------------------------

#[test]
fn approx_policy_labels_estimate_and_never_claims_exact() {
    let opts = CountOptions {
        approx: ApproxPolicy::AllowApprox {
            proxy_encoding: ProxyEncoding::O200kBase,
            reason: ApproxReason::UnsupportedModel,
        },
        ..CountOptions::default()
    };
    // An unknown model would normally error; with approx allowed it falls back.
    let result = count("hello world", "totally-unknown-model", &opts).unwrap();
    assert!(result.tokens > 0);
    assert_eq!(result.accuracy, Accuracy::Approximate);
    assert_ne!(result.accuracy, Accuracy::Exact);
    assert_eq!(result.strategy, Strategy::Approx);
    let approximation = result.approximation.expect("approximation metadata");
    assert_eq!(approximation.proxy_encoding, ProxyEncoding::O200kBase);
    assert_eq!(approximation.reason, ApproxReason::UnsupportedModel);
}

#[test]
fn exact_only_policy_surfaces_unknown_model_error() {
    let err = count("hello", "totally-unknown-model", &CountOptions::default()).unwrap_err();
    assert!(matches!(err, TokntError::UnknownModel { .. }));
}

// ---- UTF-8 validation ------------------------------------------------------

#[test]
fn invalid_utf8_is_rejected_with_offset() {
    // 0xFF is never valid UTF-8.
    let err = count_bytes(
        &[0x68, 0x69, 0xff],
        "stdin",
        "o200k_base",
        &CountOptions::default(),
    )
    .unwrap_err();
    match err {
        TokntError::InvalidUtf8 { offset, what } => {
            assert_eq!(offset, 2);
            assert_eq!(what, "stdin");
        }
        other => panic!("expected InvalidUtf8, got {other:?}"),
    }
}
