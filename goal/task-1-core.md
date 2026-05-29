# toknt — Goal 1: Counting Engine

> **First of two handoffs** (`/goal goal/task-1-core.md`). Build the accurate
> counting **core** and prove it against the proof targets; the full CLI is
> **Goal 2** (`goal/task-2-cli.md`), a separate run on top of this proven core.
> Run until this Definition of Done is met. You own the **how** within the
> decisions fixed here.
>
> **Implementation companion — read alongside this spec:**
> [`goal/research.md`](research.md) pre-resolves the build-time "confirm this"
> gaps: pinned crate versions + a ready `Cargo.toml`, the Anthropic/Gemini HTTP
> contracts, the model→strategy registry seed, and the cache/offline decisions.
> Re-confirm anything time-sensitive against the canonical URLs it lists.

## Mission

Build the token-counting **core** of `toknt` as a reusable Rust library, and
prove it works end-to-end across all four provider strategies (tiktoken,
open-weight, Anthropic, Gemini). The proof is a committed
**`verification.json`** listing each target and the token count it produced for
a shared sample.

Do **not** build the user-facing CLI here (arg parsing, output formats,
subcommands, completions, comparison tables, file/dir walking) — that is Goal 2.
Build only the library plus the minimal harness needed to emit the verification
artifact.

## Why Rust (context)

The decisive constraint: the _only_ hard part is **in-process, offline, exact
tokenization** for OpenAI and arbitrary open-weight models — the Claude/Gemini
paths are trivial HTTP. Rust owns that hard part natively (the `tokenizers`
crate **is** the reference implementation transformers.js reimplements;
`tiktoken-rs` covers OpenAI), produces the smallest dependency-free binary, and
is fastest. Hence Rust, not the TypeScript scaffold the repo shipped with.

---

## What a count means (the counting basis)

A token count is only well-defined relative to a model's tokenizer **and** a
counting basis. The default basis is **raw content**: the tokens of the input
text itself — no chat template, system prompt, or role wrapping. The basis is
fixed, but each strategy realizes it differently, and those differences are real
and must be surfaced, never hidden:

- **tiktoken / open-weight (offline, exact):** encode the raw bytes and count
  the tokens. For tokenizers with special/BOS/EOS tokens (most open-weight
  models), raw mode encodes with **`add_special_tokens = false`** — pure
  content, no BOS/EOS/template tokens. For tiktoken, use the **ordinary**
  encoding path (`encode_ordinary`-equivalent) so `<|endoftext|>`-like
  substrings are counted as ordinary text and never raise. Structural/special
  tokens are not part of the raw count; structured prompt counting is out of
  scope for these handoffs.
- **anthropic / gemini (API, provider count/estimate):** the providers expose no
  raw encoder, only a count over a request payload. Wrap the input as a single
  `user` message; the returned number is the provider's reported input-token
  count for that payload. Anthropic documents this as an estimate that can
  include system-added tokens; Gemini returns a provider count for the supplied
  content payload. Both **necessarily include a small, fixed per-request message
  envelope** the provider adds. This is inherent to the only available method,
  not a bug — record it as-is; do not subtract or hide it.

**What "exact" means here:** faithful to that model's own tokenizer on the
stated basis. Offline tokenizer paths (`tiktoken`, open-weight HF tokenizers)
are exact raw-content counts. API paths are authoritative provider
counts/estimates for the submitted payload, not exact raw-content counts. These
bases and accuracy labels are tracked per-result in `verification.json`
(`raw-content` vs `content+envelope`; `exact` vs `provider-count` /
`provider-estimate`).

---

## Proof targets

All ten were verified to exist and be reachable on 2026-05-29, except
`o200k_base`, which is a built-in tiktoken encoding rather than a hosted model.
Together they cover every strategy:

| Model                           | Strategy        | Notes                                                                 |
| ------------------------------- | --------------- | --------------------------------------------------------------------- |
| `o200k_base`                    | **tiktoken**    | Offline raw encoding target; stable proof of the OpenAI/tiktoken path |
| `claude-opus-4-8`               | **anthropic**   | Live `count_tokens`; provider-estimate; basis `content+envelope`      |
| `gemini-3.5-flash`              | **gemini**      | Live `countTokens`; provider-count; basis `content+envelope`          |
| `google/gemma-4-31B-it`         | **open-weight** | HF `tokenizer.json` (reachable with current `HF_TOKEN`)               |
| `XiaomiMiMo/MiMo-V2.5`          | **open-weight** | HF `tokenizer.json`                                                   |
| `XiaomiMiMo/MiMo-V2.5-Pro`      | **open-weight** | HF `tokenizer.json`                                                   |
| `deepseek-ai/DeepSeek-V4-Flash` | **open-weight** | HF `tokenizer.json`                                                   |
| `deepseek-ai/DeepSeek-V4-Pro`   | **open-weight** | HF `tokenizer.json`                                                   |
| `Qwen/Qwen3.5-27B`              | **open-weight** | HF `tokenizer.json`                                                   |
| `Qwen/Qwen3.5-9B`               | **open-weight** | HF `tokenizer.json`                                                   |

> OpenAI model aliases are part of the product UX and must work when they can be
> mapped correctly. The proof row uses the raw `o200k_base` encoding so Goal 1
> does not depend on one hosted OpenAI model id, but the registry must still
> seed confirmed OpenAI aliases from the current OpenAI/tiktoken docs. Do not
> invent aliases: if a requested OpenAI model is unknown or cannot be mapped to
> the correct encoding, error clearly (the `--approx` fallback is Goal 2) rather
> than silently using a proxy encoding. Model **size** (31B, Pro, Flash) is
> irrelevant for open-weight entries — only `tokenizer.json` (a few MB to tens
> of MB), never weights, is fetched.

---

## Definition of Done

The run is complete when **all** of the following hold:

- [ ] The repo is a Rust Cargo project; the counting core lives in a **library**
      (`lib.rs`), unit-testable and reusable **without any CLI**.
- [ ] All four strategies are implemented and routed: **tiktoken**,
      **open-weight** (HF), **anthropic**, **gemini**.
- [ ] **`verification.json` is committed and fully populated for all ten proof
      targets** — each entry carries `model`, `strategy`, `basis`, `accuracy`,
      `tokens`, and `approximation` metadata (null for non-approximate counts;
      plus `encoding` / `add_special_tokens` / HF `revision` where applicable).
      In this repo the keys are present (mise loads `.env`), so the live entries
      MUST be populated; they skip cleanly only in a keyless environment.
- [ ] **Correctness guards** (unit/integration tests) pass: - empty input → `0`
      tokens for every offline tokenizer; - non-empty input → `> 0`; - encoding
      selection is right (`o200k_base` routes directly; confirmed modern OpenAI
      aliases such as `gpt-5.2` route to `o200k_base`; legacy OpenAI aliases
      such as `gpt-4` / `gpt-3.5-turbo` route to `cl100k_base`, not
      `o200k_base`); - open-weight raw uses `add_special_tokens = false` — a
      known short input proves no stray BOS/EOS is counted.
- [ ] `cargo build --release` is clean; `cargo clippy` is clean;
      `cargo fmt --check` passes.
- [ ] The superseded TypeScript/Node scaffold is removed and repo tooling (mise,
      Taskfile, git/claude config) is adapted to Rust.
- [ ] The library API is shaped so **Goal 2 can build the CLI on top without
      reshaping the core**: clean count request/options types, explicit
      network/offline policy, explicit approximation policy, and a result shape
      like
      `count(input, model, options) -> {tokens, basis, strategy, accuracy,     approximation, …}`;
      strategies sit behind a common trait/enum.

**Explicitly NOT in this goal** (deferred to Goal 2): CLI arg parsing,
`--json`/`-v`/`--stats` output formats, subcommands (`models`/`pull`/`cache`/
`completions`), multi-model comparison tables, file/glob/directory walking,
`--approx`, `--offline` as user-facing flags, shell completions, README. Do
**not** defer the core result metadata or network/approximation policy types
described below; Goal 2 should only wire those APIs to flags.
Distribution/CI/packaging and cost estimation remain out of scope entirely.

---

## Provider strategies

| Strategy        | Method                                        | Network          | Basis            | Accuracy          |
| --------------- | --------------------------------------------- | ---------------- | ---------------- | ----------------- |
| **tiktoken**    | `tiktoken-rs`, by encoding (ordinary path)    | No               | raw-content      | exact             |
| **open-weight** | `tokenizers` crate on cached `tokenizer.json` | First fetch only | raw-content      | exact             |
| **anthropic**   | `POST /v1/messages/count_tokens`              | Yes              | content+envelope | provider-estimate |
| **gemini**      | `:countTokens`                                | Yes              | content+envelope | provider-count    |

Implementation notes:

- **tiktoken:** count via the ordinary/no-special path (see basis). Resolve the
  model to an encoding via the registry/encoding rules below.
- **open-weight:** `encode(text, add_special_tokens = false)`; count `.ids`.
  Fetch `tokenizer.json` (+ `tokenizer_config.json`) once and cache; record the
  resolved HF **revision** so the count is reproducible.
- **anthropic:** `x-api-key` + `anthropic-version` headers; body
  `{model, messages:[{role:"user", content:<text>}]}`; read `input_tokens`;
  label the result `accuracy = provider-estimate`.
- **gemini:** `models/{model}:countTokens` with `x-goog-api-key` preferred (the
  query-string `?key=` form is also supported by Google); body
  `{contents:[{parts:[{text:<text>}]}]}`; read `totalTokens`; label the result
  `accuracy = provider-count`.
- **Verify the current request/response contracts against official docs at build
  time** — these endpoints evolve; the shapes above may be stale. Prefer raw
  `reqwest` (with **rustls**, not native-tls/OpenSSL) over provider SDKs to keep
  the binary lean and the future static build trivial. See References.

---

## Library API requirements for Goal 2

Goal 1 must expose the policy and metadata surface the CLI will need, even
though the user-facing flags are deferred:

- `CountOptions` (or equivalent) includes a **network policy**: `allow-network`
  vs `offline`. `offline` means no HTTP request may be attempted: tiktoken
  works, cached HF tokenizers work, uncached HF tokenizers return an actionable
  cache-miss error, and Anthropic/Gemini return an actionable network-required
  error before constructing a request.
- `CountOptions` includes an **approximation policy**: `exact-only` vs
  `allow-approx { proxy_encoding, reason }`. Goal 2's `--approx` flag should map
  to this rather than implementing approximation in the CLI formatter.
- `CountResult` includes `tokens`, `model`, `resolved_model`, `strategy`,
  `basis`, `accuracy`, and `approximation`. Use these values:
  `accuracy = exact | provider-count | provider-estimate | approximate`;
  `approximation = null` for exact/provider results, otherwise include at least
  `proxy_encoding` (`o200k_base` or `cl100k_base`) and a short `reason`
  (`no-model`, `missing-api-key`, `unsupported-model`, etc.).
- Approximate counts must never reuse `accuracy = exact`, even when the proxy
  tokenizer itself is exact for its own encoding.

---

## Model resolution

Resolve a model id to a strategy in this order:

1. **Explicit prefix** — `openai:`, `anthropic:`, `google:`, `hf:` forces the
   strategy (e.g. `hf:org/model`, `openai:o200k_base`).
2. **Alias registry** — friendly names → concrete target/encoding. Seed at least
   the proof set's API entries: `claude-opus-4-8 → anthropic`,
   `gemini-3.5-flash → gemini`. Also seed confirmed OpenAI model aliases from
   the current OpenAI/tiktoken docs, including current GPT-5 family names such
   as `gpt-5`, `gpt-5.1`, and `gpt-5.2` to `o200k_base`, plus legacy families
   such as `gpt-4` / `gpt-3.5-turbo` to `cl100k_base`. Keep this as explicit
   registry data or exact prefix rules; do not use a blanket
   `gpt-* → o200k_base` rule.
3. **Bare HF id** — anything containing `/` → open-weight (this routes all seven
   HF models above without registry entries).
4. **Raw encoding name** — `o200k_base` / `o200k_harmony` / `cl100k_base` /
   `p50k_base` / `r50k_base` / `gpt2` → tiktoken directly.
5. Otherwise → actionable error (name the prefix forms and that the model is
   unknown).

The registry is **data**, small and easy to extend; the prefix and HF-id escape
hatches mean unknown/new models still work.

---

## Tokenizer cache & network policy

- Open-weight tokenizers fetch on first use from the HF Hub and cache under a
  toknt-owned XDG cache dir (respect `TOKNT_CACHE_DIR`); later runs are offline
  & instant. Use `hf-hub` for clean control of cache location, `HF_TOKEN` auth
  (gated models), and offline behavior. The library should expose enough status
  for Goal 2 to print a one-time "fetching..." notice, but it should not
  unconditionally write to stderr itself.
- Expose narrow cache helpers for Goal 2: prefetch/pull an open-weight
  tokenizer, return the cache path, list cached tokenizer entries, and remove a
  cached tokenizer entry. These are part of the core boundary, not CLI
  formatting.
- The library-level `offline` policy is a hard no-network contract, not just a
  CLI convenience. Check the cache before constructing HF/API clients, and
  return a typed error that Goal 2 can render as `toknt pull <model>` or
  `--offline cannot count API-backed model <model>`.
- **Do not** embed open-weight `tokenizer.json` files in the binary. This rule
  is about HF tokenizer files only — `tiktoken-rs`'s compiled-in BPE ranks are
  expected and correct, and are what makes the tiktoken path offline.

---

## Environment

- Env: `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, `OPENAI_API_KEY` (only if you add
  an OpenAI cross-check), `HF_TOKEN`, `TOKNT_CACHE_DIR`.
- In this repo, **mise loads `.env` automatically** (`.mise.toml`) and exports
  it to child processes, so all keys (incl. `HF_TOKEN`) are available during
  build, tests, and live verification. Run commands via mise (`mise exec -- ...`
  / a task) so the keys reach the process.

---

## Verification harness (the gate)

**Use the sample files already committed** under `tests/fixtures/samples/` —
reuse them, don't regenerate (they're crafted for reproducible counts): a
representative mixed sample (`sample.txt`: English + code + unicode/emoji + CJK,
a few hundred tokens), `empty.txt`, and `special-tokens.txt` (contains literal
`<|endoftext|>` / `<|im_start|>` strings, to prove ordinary-path counting). Read
the **exact bytes** of these files (don't re-type strings), then validate that
inputs are UTF-8 text before counting. Recreate one only if it is genuinely
missing.

Emit **`verification.json`** at the repo root via an explicit live verification
harness that drives the library over the proof set for the primary sample. Use a
clear command such as `mise exec -- cargo run --bin toknt-verify` (or a Taskfile
wrapper like `task verify-live`). Normal `cargo test` should run correctness
guards and must not silently rewrite the committed artifact; the live
verification command is the artifact-producing gate.

```json
{
  "generated_at": "2026-05-29",
  "sample": {
    "path": "tests/fixtures/samples/sample.txt",
    "bytes": 1234,
    "sha256": "…"
  },
  "results": [
    {
      "model": "o200k_base",
      "strategy": "tiktoken",
      "encoding": "o200k_base",
      "basis": "raw-content",
      "accuracy": "exact",
      "approximation": null,
      "tokens": 312
    },
    {
      "model": "claude-opus-4-8",
      "strategy": "anthropic",
      "basis": "content+envelope",
      "accuracy": "provider-estimate",
      "approximation": null,
      "tokens": 320
    },
    {
      "model": "gemini-3.5-flash",
      "strategy": "gemini",
      "basis": "content+envelope",
      "accuracy": "provider-count",
      "approximation": null,
      "tokens": 318
    },
    {
      "model": "deepseek-ai/DeepSeek-V4-Pro",
      "strategy": "open-weight",
      "revision": "<sha>",
      "add_special_tokens": false,
      "basis": "raw-content",
      "accuracy": "exact",
      "approximation": null,
      "tokens": 305
    }
  ]
}
```

The real artifact includes all ten proof targets; the example above is truncated
for readability.

The live verification command: (1) writes/refreshes `verification.json`; (2)
asserts every proof target produced a count (live entries skip-clean only
without keys — here they must populate); (3) reuses the same core counting path
the CLI will call. A green live run with a fully-populated `verification.json`
is the pass point. The artifact is committed so it can be inspected.

> Note: For the API strategies, toknt is a passthrough — the authoritative test
> is **live provider parity** (toknt's number == a fresh independent API call),
> not a frozen value. Treat the recorded API counts as a drift reference, not a
> golden, and keep their `accuracy` labels as provider count/estimate rather
> than exact.

---

## Errors

Even without the CLI, the core's errors must be specific and actionable, since
Goal 2 will surface them:

- Unknown model → name the prefix forms and that it's unrecognized.
- Missing API key → `claude requires ANTHROPIC_API_KEY` (and which env var).
- Gated HF model without access → hint `HF_TOKEN` + license acceptance.
- Unknown/unsupported OpenAI model alias or tiktoken encoding → say so
  explicitly.
- Invalid UTF-8 input → say which path/input is invalid; do not silently
  lossy-decode.

Use `anyhow`/`thiserror` (or your choice) for clean error context. Avoid
unnecessary copies for large inputs, while accepting that exact tokenizer APIs
usually need the full validated UTF-8 text.

---

## Repo migration

The repo was scaffolded with a TypeScript/Node toolchain. **Keep it — add the
Rust project alongside; do not strip the Node tooling.** `package.json`,
`package-lock.json`, `node_modules/`, `eslint.config.js`, `.prettierrc.js`, and
`.prettierignore` stay: they back the prettier/eslint formatting setup (and
editor/commit hooks) used on the repo's Markdown and configs. The shipped
`toknt` is a self-contained Rust binary regardless — the Node files are
dev-tooling only, never part of the artifact.

- **Add** the Rust project: `Cargo.toml`, `src/lib.rs` (the counting core) plus
  a thin `src/main.rs`/verify harness, with a clean module layout. Any
  pre-existing `Cargo.toml`/`Cargo.lock`/`src/`/`target/` is a throwaway
  LSP/setup probe — non-authoritative; (re)scaffold properly.
- **Adapt** tooling: add the Rust toolchain to `.mise.toml` (keep the existing
  `node`/`bun` entries). The current `Taskfile.yaml` is unrelated-project
  boilerplate (TS/eslint/prettier/bun/`giget` tasks pointing at _other_ repos) —
  **rewrite it for cargo** (build/test/clippy/fmt/run); you may keep a
  Markdown-format task since prettier stays. Keep `.gitignore` (`/target` and
  `node_modules/` are already covered), `.mcp.json`, `.claude/`, and
  `rust-toolchain.toml` in sync with the mise Rust pin.

---

## References

Confirm current API/library contracts before implementing — these may have
changed since this was written.

- Anthropic token counting:
  https://platform.claude.com/docs/en/build-with-claude/token-counting
- Gemini token counting: https://ai.google.dev/gemini-api/docs/tokens
- `tiktoken-rs`: https://github.com/zurawiki/tiktoken-rs
- HF `tokenizers` (Rust): https://github.com/huggingface/tokenizers
- `hf-hub` (Rust): https://github.com/huggingface/hf-hub
- Implementation quick-reference (crates, HTTP contracts, registry seed, cache):
  [`goal/research.md`](research.md)

---

## Working agreement

- Commit in logical, reviewable increments; branch off `main` (don't commit
  straight to `main` for large changes).
- Keep `cargo clippy` and `cargo fmt` clean as you go.
- When a fixed decision here proves wrong or impossible, **stop and surface it**
  rather than silently diverging.
- Match surrounding code idiom and density. Favor a small, sharp core over
  feature bloat. Leave the library boundary clean for Goal 2.
