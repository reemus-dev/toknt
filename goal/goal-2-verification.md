# Goal 2 verification — 2026-09-12

Goal 2's CLI completion gates passed on macOS with Rust 1.96.0. Commands ran
through mise so the saved project's `.env` credentials reached child processes.

## Automated and live gates

| Check | Result |
| --- | --- |
| `cargo test` | 59 passed: 7 library, 1 renderer, 40 CLI, 11 correctness; none skipped |
| `cargo build --release` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| Prettier check of the updated documentation | Passed |
| `cargo run --bin toknt-verify` | All 10 proof targets populated; none skipped |

The committed [verification.json](../verification.json) contains the shared
sample's counts, basis, accuracy, and tokenizer metadata. Normal tests did not
modify it. The live refresh reproduced every count and metadata value; its
verification date is 2026-09-12.

## Release-binary checks

All 57 manual checks passed, including expected nonzero exits for invalid input
and disallowed operations. Cache mutations used an isolated temporary
`TOKNT_CACHE_DIR`; the normal tokenizer cache was not removed.

| Surface | Exercised behavior |
| --- | --- |
| Help and completions | Root and every subcommand's help; bash, zsh, fish, PowerShell completions |
| Input and precedence | Inline text, stdin, a file, missing file, invalid UTF-8, terminal without input, text vs stdin, text/file conflict, environment model and flag override |
| Strategies | tiktoken, cached and freshly pulled HF tokenizers, live Anthropic, live Gemini |
| Output | Bare integer, JSON, verbose, stats, per-file totals, multi-model comparison with explicit basis labels |
| Trust controls | Missing model, no-model approximation, exact counts retained with `--approx`, missing-key guidance/fallback, offline API and uncached-HF rejection with and without `--approx` |
| Filesystem | Multiple files, quoted glob, recursive directories, `.gitignore`, `--no-ignore`, `--hidden` |
| Subcommands | Model listing/search; isolated `pull`, `cache path`, `cache ls`, and `cache rm` |
| Provider parity | Fresh independent calls matched CLI counts: Anthropic 453; Gemini 304 |

## Completion regressions

- Multi-file/multi-model `--stats` now labels each model's tokens-per-word.
  On `sample.txt` plus `special-tokens.txt`, `o200k_base` totals 411 tokens and
  2.02 tokens/word; `cl100k_base` totals 437 and 2.15. Regression coverage also
  checks weighted partial totals, unavailable cells, and approximation markers.
- Offline API and uncached-HF errors do not recommend an ineffective
  `--approx` rerun. API errors retain network guidance; HF misses retain the
  `toknt pull` hint. The strict offline policy remains in force with `--approx`.
- JSON totals expose distinct `bases`, `accuracies`, and `approximations` from
  successful contributors. A missing-key Claude fallback across the same two
  samples totals 411, explicitly approximate, using the `o200k_base` proxy.
  Regression coverage distinguishes partial/all-failed columns from successful
  zero-token counts. A renderer test checks mixed provider/proxy contributors
  and deduplication without depending on changing live credentials.

The [README](../README.md#output) documents the additive aggregate schema and
ratio semantics. Individual count metadata and the counting engine are unchanged
by the final CLI fixes.
