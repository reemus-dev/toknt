# toknt — implementation quick-reference

Companion to the Goal 1 and Goal 2 specs ([`task-1-core.md`](task-1-core.md),
[`task-2-cli.md`](task-2-cli.md)). This doc pre-resolves the "confirm at build
time" gaps in the specs: crate APIs, provider HTTP contracts, the model→strategy
registry, cache/XDG choices, and the test-vector setup.

**How to use it:** the inline facts are a fast path; the **canonical URL on each
section is the source of truth.** Anything time-sensitive (model IDs, API
versions, crate versions) should be re-confirmed against its URL before you rely
on it — see the staleness flags.

> **Staleness:** verified 2026-05-29. Crate versions and provider model IDs
> move. The two endpoints (Anthropic, Gemini) are explicitly called out in
> `task-1-core.md` as "may be stale — verify against docs." Re-check the four "⚠
> verify" items below before shipping.

---

## 0. Crate stack — versions + Cargo.toml

Starting point confirmed against docs.rs / crates.io, 2026-05-29. Before
committing `Cargo.toml`, re-check exact latest versions and feature names;
prefer the smallest feature set that preserves tokenizer correctness and
rustls-only HTTP.

```toml
[dependencies]
tiktoken-rs = "0.11"                                                  # OpenAI BPE, embedded/offline
tokenizers  = "0.23"                                                  # HF; do not enable its optional `http` feature
hf-hub      = { version = "0.5", default-features = false, features = ["ureq", "rustls-tls"] }
clap        = { version = "4.6", features = ["derive"] }
clap_complete = "4.6"
reqwest     = { version = "0.13", features = ["json", "blocking", "rustls-tls"], default-features = false }
etcetera    = "0.11"                                                  # XDG-style cache paths
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
anyhow      = "1"
thiserror   = "2"                                                     # note: v2, not v1
```

| Crate         | docs.rs                       | crates.io                              |
| ------------- | ----------------------------- | -------------------------------------- |
| tiktoken-rs   | https://docs.rs/tiktoken-rs   | https://crates.io/crates/tiktoken-rs   |
| tokenizers    | https://docs.rs/tokenizers    | https://crates.io/crates/tokenizers    |
| hf-hub        | https://docs.rs/hf-hub        | https://crates.io/crates/hf-hub        |
| clap          | https://docs.rs/clap          | https://crates.io/crates/clap          |
| clap_complete | https://docs.rs/clap_complete | https://crates.io/crates/clap_complete |
| reqwest       | https://docs.rs/reqwest       | https://crates.io/crates/reqwest       |
| etcetera      | https://docs.rs/etcetera      | https://crates.io/crates/etcetera      |
| anyhow        | https://docs.rs/anyhow        | https://crates.io/crates/anyhow        |
| thiserror     | https://docs.rs/thiserror     | https://crates.io/crates/thiserror     |

Build for a self-contained binary: prefer **rustls** everywhere (no OpenSSL).
Keep HF downloads on a **sync** path if the current `hf-hub` supports it, and do
not enable `tokenizers`' optional `http` feature. Do not disable `tokenizers`
default features unless you have proven all proof-set `tokenizer.json` files
still load and count correctly.

---

## 1. OpenAI strategy — `tiktoken-rs` (offline, exact)

Docs: https://docs.rs/tiktoken-rs · Repo:
https://github.com/zurawiki/tiktoken-rs

**Key fact for the spec's "do not embed tokenizers" rule:** tiktoken-rs _does_
embed its `.tiktoken` rank files in the binary via `include_str!` — that's
intended and correct. The "don't embed" rule is about the multi-MB **HF**
tokenizers, not OpenAI's tiny rank tables. OpenAI counting is fully offline with
zero downloads.

```rust
use tiktoken_rs::{o200k_base_singleton, bpe_for_model};

let bpe = o200k_base_singleton();          // singleton: parse cost paid once
let count = bpe.encode_ordinary(text).len();   // raw content → no special tokens

// or resolve via model name:
let bpe = bpe_for_model("gpt-4o")?;        // Err on unknown model, no fuzzy match
```

- Use `*_singleton()` variants — the non-singleton `o200k_base()` re-parses
  (~100ms) each call.
- **`encode_ordinary`** = raw count (special tokens as literal text) → the
  default per spec. `encode_with_special_tokens` counts `<|endoftext|>` etc. as
  single tokens.
- Encodings exposed: `o200k_harmony`, `o200k_base`, `cl100k_base`, `p50k_base`,
  `p50k_edit`, `r50k_base`/`gpt2`. The raw-encoding-name path in
  `task-1-core.md`'s resolution maps straight onto these.

---

## 2. Open-weight strategy — `hf-hub` + `tokenizers` (download once, then offline)

Docs: https://docs.rs/hf-hub · https://docs.rs/tokenizers

**Decision (resolves the spec's open question):** download with **`hf-hub`**
(clean cache-dir + token control), then load the file with
**`tokenizers::Tokenizer::from_file`**. Do _not_ enable `tokenizers`' `http`
feature or use its `from_pretrained` — that hides network calls and bypasses
your cache policy.

```rust
use hf_hub::api::sync::ApiBuilder;
use tokenizers::Tokenizer;

let api = ApiBuilder::new()
    .with_cache_dir(cache_dir)            // from TOKNT_CACHE_DIR or XDG (see §8)
    .with_token(hf_token)                 // Option<String>, for gated repos
    .with_progress(false)
    .build()?;
let path = api.model(repo_id).get("tokenizer.json")?;   // cache-first; downloads if absent
let tok = Tokenizer::from_file(path).map_err(...)?;
let count = tok.encode(text, /* add_special_tokens */ false)?.get_ids().len();
```

- Files to fetch: **`tokenizer.json`** (the self-contained one) + optionally
  `tokenizer_config.json` (~3 KB; useful for future special-token logic).
- `encode_fast(text, false)` skips offset computation — slightly faster when you
  only need the count.
- **`--offline` gotcha:** hf-hub (v0.5) has **no built-in offline flag**.
  `get()` will hit the network if the file is missing. Implement offline
  yourself: use the `Cache` struct to compute the expected path, check
  existence, and error with the `toknt pull <model>` hint _before_ constructing
  the `Api`.
- **Auth:** `with_token(Some(...))` for gated models; `HF_TOKEN` env and
  `~/.cache/huggingface/token` are read by `ApiBuilder::from_env()`.

---

## 3. Anthropic strategy — Count Tokens API ⚠ verify

**Docs (source of truth):**

- Guide: https://platform.claude.com/docs/en/build-with-claude/token-counting
- API ref: https://platform.claude.com/docs/en/api/messages-count-tokens
- Errors: https://platform.claude.com/docs/en/api/errors

GA endpoint (no beta header). Counting is free, with its own rate limits.

```
POST https://api.anthropic.com/v1/messages/count_tokens
Headers:  x-api-key: $ANTHROPIC_API_KEY
          anthropic-version: 2023-06-01        # current
          content-type: application/json
Body:     {"model":"claude-sonnet-4-6","messages":[{"role":"user","content":"<text>"}]}
Response: {"input_tokens": 14}                  # the only field
```

- **Errors** use
  `{"type":"error","error":{"type":"...","message":"..."},"request_id":"..."}`.
  Map by HTTP status: 400 `invalid_request_error`, 401 `authentication_error`,
  403 `permission_error`, 404 `not_found_error`, 413 `request_too_large`, 429
  `rate_limit_error`, 5xx `api_error`/`overloaded_error`.
- Request size cap: **32 MB**. Rate limits by tier: 100 / 2k / 4k / 8k RPM.
- **Accuracy/drift:** docs say count is "an estimate… may differ by a small
  amount," and may include system-added tokens you aren't billed for. Wrapping
  raw text as a single `user` message also adds a few template tokens — surface
  this in `--verbose` rather than pretending it's the bare content count.

---

## 4. Gemini strategy — countTokens API ⚠ verify

**Docs (source of truth):**

- Guide: https://ai.google.dev/gemini-api/docs/tokens
- REST ref: https://ai.google.dev/api/tokens#method:-models.counttokens

`v1beta` only (no `v1` for this method).

```
POST https://generativelanguage.googleapis.com/v1beta/models/{model}:countTokens
Auth:     header  x-goog-api-key: $GEMINI_API_KEY     # preferred (keeps key out of logs)
          — or —  ?key=$GEMINI_API_KEY                # query param, also supported
Body:     {"contents":[{"parts":[{"text":"<text>"}]}]}
Response: {"totalTokens":10,"promptTokensDetails":[{"modality":"TEXT","tokenCount":10}], ...}
```

- Primary field: **`totalTokens`**. Also returns `cachedContentTokenCount` and
  per-modality `promptTokensDetails` (0 for plain text).
- **Errors:** Google format
  `{"error":{"code":400,"message":"...","status":"INVALID_ARGUMENT"}}`.
- Same wrap-as-message overhead caveat as Anthropic.

---

## 5. Model registry seed data

`task-1-core.md` leaves the registry as a build-time task. Seed tables below;
**re-confirm provider model IDs against their model-list pages — these churn
fastest.**

### 5a. OpenAI model → encoding (⚠ the live truth is one file)

Source of truth: https://github.com/openai/tiktoken/blob/main/tiktoken/model.py
(`MODEL_PREFIX_TO_ENCODING` / `MODEL_TO_ENCODING`). Cookbook:
https://cookbook.openai.com/examples/how_to_count_tokens_with_tiktoken

| Encoding           | Model families (prefixes)                                                                                                   |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| `o200k_harmony`    | `gpt-oss-*` (open-weight; superset of o200k_base + harmony tokens)                                                          |
| `o200k_base`       | current modern OpenAI families such as `gpt-5*`, `gpt-4.1*`, `gpt-4.5*`, `gpt-4o*`, `chatgpt-4o*`, `o1*`, `o3*`, `o4-mini*` |
| `cl100k_base`      | `gpt-4*`, `gpt-3.5-turbo*`, `text-embedding-ada-002`, `text-embedding-3-{small,large}`                                      |
| `p50k_base`        | `text-davinci-002/003`, `code-davinci-*` (deprecated)                                                                       |
| `r50k_base`/`gpt2` | `davinci`/`curie`/`babbage`/`ada`, `gpt2` (deprecated/legacy)                                                               |

Because the raw-encoding-name path (`-m o200k_base`) is in the resolver, the
tiktoken proof row does not depend on a hosted OpenAI model id. Add OpenAI model
aliases only after checking the current tiktoken model map; do not apply a broad
`gpt-* -> o200k_base` rule because older GPT families use other encodings.

### 5b. Anthropic model IDs (⚠ verify)

Overview: https://platform.claude.com/docs/en/about-claude/models/overview

Current generation (2026-05-29): `claude-opus-4-8`, `claude-sonnet-4-6`,
`claude-haiku-4-5` (pinned `claude-haiku-4-5-20251001`). Still-active legacy:
`claude-opus-4-7/4-6/4-5/4-1`, `claude-sonnet-4-5`. Deprecated, **retiring
2026-06-15**: `claude-sonnet-4-20250514`, `claude-opus-4-20250514`. From the 4.6
generation, dateless IDs are pinned snapshots (not evergreen). You don't need to
hardcode every ID — the API validates the model string; the registry just needs
the proof-set alias from `task-1-core.md` (`claude-opus-4-8` → anthropic
strategy), plus any additional convenient aliases confirmed during the build.

### 5c. Gemini model IDs (⚠ verify — fastest-moving)

Models: https://ai.google.dev/gemini-api/docs/models Runtime enumeration:
`GET https://generativelanguage.googleapis.com/v1beta/models?key=$GEMINI_API_KEY`

Stable (2026-05-29): `gemini-3.5-flash`, `gemini-3.1-flash-lite`,
`gemini-2.5-pro`, `gemini-2.5-flash`, `gemini-2.5-flash-lite`. Preview:
`gemini-3.1-pro-preview`, `gemini-3-flash-preview`. Deprecated:
`gemini-2.0-flash*`. Given the churn, prefer validating against the live models
list over a hardcoded table.

### 5d. Open-weight HF repos

URL patterns:

- Tree: `https://huggingface.co/{repo}/tree/main`
- Raw file: `https://huggingface.co/{repo}/resolve/main/tokenizer.json`

| Proof repo                      | Notes                                                       |
| ------------------------------- | ----------------------------------------------------------- |
| `google/gemma-4-31B-it`         | Verify access with current `HF_TOKEN`; fetch tokenizer only |
| `XiaomiMiMo/MiMo-V2.5`          | Verify `tokenizer.json` exists before committing proof      |
| `XiaomiMiMo/MiMo-V2.5-Pro`      | Verify `tokenizer.json` exists before committing proof      |
| `deepseek-ai/DeepSeek-V4-Flash` | Verify `tokenizer.json` exists before committing proof      |
| `deepseek-ai/DeepSeek-V4-Pro`   | Verify `tokenizer.json` exists before committing proof      |
| `Qwen/Qwen3.5-27B`              | Verify `tokenizer.json` exists before committing proof      |
| `Qwen/Qwen3.5-9B`               | Verify `tokenizer.json` exists before committing proof      |

The proof set intentionally uses the same seven HF ids as `task-1-core.md`. If
one has disappeared, is renamed, or no longer exposes a usable `tokenizer.json`,
stop and surface it instead of silently substituting a different model. Gating
drives the spec's dedicated `HF_TOKEN` error path.

---

## 6. Caching & XDG paths

`task-1-core.md` pins `TOKNT_CACHE_DIR` as the tokenizer-cache override. There
is no persistent config file in these handoffs; model selection comes from
`-m/--model` or `TOKNT_MODEL`.

- **`etcetera`** (https://docs.rs/etcetera): `choose_app_strategy(...)` gives
  XDG-style paths; use its cache directory for toknt-owned tokenizer files.

Resolution order for the tokenizer cache: `TOKNT_CACHE_DIR` env → XDG cache dir
from `etcetera` → pass to `ApiBuilder::with_cache_dir`.

---

## 7. Testing & cross-validation

Spec DoD = normal correctness tests plus an explicit live verification command
that refreshes and validates `verification.json` where keys are present (this
repo loads keys via mise). Live verification may skip cleanly in keyless
environments, but in this repo the proof artifact must be fully populated.

**Generate golden counts from reference impls:**

- OpenAI: `tiktoken` (Python) — https://pypi.org/project/tiktoken/ ·
  `tiktoken.encoding_for_model("gpt-4o").encode(text)`
- Open-weight: `tokenizers` (Python) — https://pypi.org/project/tokenizers/ — or
  `transformers` `AutoTokenizer`. JS alts: `gpt-tokenizer`
  (https://github.com/niieani/gpt-tokenizer), transformers.js.

**Edge cases to vector (from spec):** empty, ASCII, unicode/emoji, CJK, code,
trailing newlines, very large input.

**Committable tokenizer fixture (avoid network in open-weight tests):**

- `google-bert/bert-base-uncased` — `tokenizer.json` ~466 KB → best small
  fixture.
- `openai-community/gpt2` — ~1.04 MB; bonus: tiktoken also maps `"gpt2"`, so you
  can cross-check tiktoken vs HF tokenizers on identical text.
- Blob views:
  `https://huggingface.co/google-bert/bert-base-uncased/blob/main/tokenizer.json`,
  `https://huggingface.co/openai-community/gpt2/blob/main/tokenizer.json`

**Capture live API fixtures now (keys are in `.env` via mise):** save one real
request/response per provider as a contract fixture if it helps implementation.
The committed `verification.json` is produced by the explicit live verification
command; normal `cargo test` should not rewrite it as a side effect.

```bash
# Anthropic
curl https://api.anthropic.com/v1/messages/count_tokens \
  -H "x-api-key: $ANTHROPIC_API_KEY" -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model":"claude-sonnet-4-6","messages":[{"role":"user","content":"Hello"}]}'
# → {"input_tokens": N}

# Gemini
curl "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:countTokens" \
  -H "x-goog-api-key: $GEMINI_API_KEY" -H "Content-Type: application/json" -X POST \
  -d '{"contents":[{"parts":[{"text":"Hello"}]}]}'
# → {"totalTokens": N, ...}
```

---

## 8. Open decisions captured here (so the implementer doesn't re-litigate)

| Spec open question                         | Resolution in this doc                                                     |
| ------------------------------------------ | -------------------------------------------------------------------------- |
| `hf-hub` vs `tokenizers` pretrained loader | `hf-hub` download → `Tokenizer::from_file` (§2)                            |
| XDG cache path / crate                     | `etcetera`; `TOKNT_CACHE_DIR` first (§6)                                   |
| `--offline` for HF                         | hf-hub has no offline flag — gate via `Cache` path-check before `Api` (§2) |
| tiktoken "embedding" vs spec rule          | tiktoken-rs embeds ranks intentionally; rule targets HF only (§1)          |
| TLS / runtime                              | rustls + sync (`ureq`) paths; avoid tokio for downloads (§0)               |

---

## 9. Reference URL index

**Crates:** docs.rs/{tiktoken-rs, tokenizers, hf-hub, clap, clap_complete,
reqwest, etcetera, anyhow, thiserror}

**OpenAI:** model map
https://github.com/openai/tiktoken/blob/main/tiktoken/model.py · cookbook
https://cookbook.openai.com/examples/how_to_count_tokens_with_tiktoken

**Anthropic:** guide
https://platform.claude.com/docs/en/build-with-claude/token-counting · API ref
https://platform.claude.com/docs/en/api/messages-count-tokens · errors
https://platform.claude.com/docs/en/api/errors · models
https://platform.claude.com/docs/en/about-claude/models/overview

**Gemini:** guide https://ai.google.dev/gemini-api/docs/tokens · REST ref
https://ai.google.dev/api/tokens#method:-models.counttokens · models
https://ai.google.dev/gemini-api/docs/models

**HuggingFace:** repo `https://huggingface.co/{repo}` · raw
`https://huggingface.co/{repo}/resolve/main/tokenizer.json`

**Reference tokenizers:** tiktoken https://pypi.org/project/tiktoken/ ·
tokenizers https://pypi.org/project/tokenizers/ · gpt-tokenizer
https://github.com/niieani/gpt-tokenizer
