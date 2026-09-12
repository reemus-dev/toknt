# toknt

A fast, lightweight CLI that prints an **accurate, model-specific token count**
for arbitrary UTF-8 text — file(s), globs, directories, inline `-t`, or piped
stdin — against one or more specific models.

Accuracy is the product. Offline tokenizers are **exact-or-error**; provider
endpoints return a labeled **count/estimate**; an approximation happens only
when you ask for one (`--approx`) and is **always marked, never silently
exact**.

## Install (from source)

Requires the Rust toolchain pinned in `rust-toolchain.toml` (1.96).

```bash
cargo build --release      # -> target/release/toknt
./target/release/toknt --help
```

Drop the binary on your `PATH` (e.g. `cp target/release/toknt ~/.local/bin/`).
It is a single self-contained binary — OpenAI BPE ranks are compiled in; Hugging
Face tokenizers download on first use into a local cache.

## Quickstart

```bash
toknt -m o200k_base file.txt                 # exact, offline -> 1423
echo "hello world" | toknt -m o200k_base     # stdin -> 2
toknt -m gpt-5.2 -t "hello world"            # OpenAI alias -> o200k_base
toknt -m deepseek-ai/DeepSeek-V4-Pro f.txt   # Hugging Face open-weight (auto-detected)
toknt -m claude-opus-4-8 file.txt            # Anthropic provider estimate (needs key)
toknt -m gpt-5.2 -m claude-opus-4-8 f.txt    # multi-model comparison table
toknt -m o200k_base src/ docs/ --stats       # recurse dirs, per-file + TOTAL, metrics
toknt --approx file.txt                      # labeled estimate, no model needed
```

A **model is required** (via `-m` or `TOKNT_MODEL`) unless you pass `--approx`.

## What a count means

The default basis is **raw content**. How that is measured depends on the
strategy, and `toknt` surfaces it rather than conflating bases:

| Strategy        | Trigger                            | Basis              | Accuracy            | Network                  |
| --------------- | ---------------------------------- | ------------------ | ------------------- | ------------------------ |
| **tiktoken**    | encoding name / OpenAI model alias | `raw-content`      | `exact`             | none (compiled-in)       |
| **open-weight** | a Hugging Face `org/name`          | `raw-content`      | `exact`             | first fetch, then cached |
| **anthropic**   | `claude*`                          | `content+envelope` | `provider-estimate` | `ANTHROPIC_API_KEY`      |
| **gemini**      | `gemini*`                          | `content+envelope` | `provider-count`    | `GEMINI_API_KEY`         |
| **approx**      | `--approx` fallback                | `raw-content`      | `approximate`       | none (tiktoken proxy)    |

Offline tokenizers count the input text only (no chat template, no BOS/EOS).
Anthropic/Gemini count the text wrapped as a single `user` message, so their
number includes a small **fixed message envelope** inherent to the count
endpoint — recorded as `content+envelope`, never subtracted or hidden. A
comparison table flags envelope rows and never implies a like-for-like delta
against a raw-content row.

## Model selection

Resolution order (first match wins):

1. **Explicit prefix** forces a strategy: `openai:`, `anthropic:`, `google:`,
   `hf:` — e.g. `hf:org/name`, `openai:cl100k_base`.
2. **OpenAI aliases** → the right encoding (`gpt-5.2`, `gpt-4o` → `o200k_base`;
   `gpt-4`, `gpt-3.5-turbo` → `cl100k_base`). Never a blanket
   `gpt-* → o200k_base` — older families use other encodings.
3. **Providers by shape**: `claude*` → Anthropic, `gemini*` → Gemini.
4. **Bare Hugging Face id**: anything containing `/` → open-weight.
5. **Raw tiktoken encoding name**: `o200k_base`, `o200k_harmony`, `cl100k_base`,
   `p50k_base`, `p50k_edit`, `r50k_base`, `gpt2`.

Run `toknt models [QUERY]` to list or search the registry. Precedence is
**flags > `TOKNT_MODEL`**; repeating `-m` produces a comparison.

`--approx` is the universal "I accept an estimate" escape hatch. It covers the
no-model and no-API-key cases via a tiktoken `o200k_base` proxy, and it only
_falls back_ — it never overrides a trusted exact or provider count when one is
available.

## Output

- **Default** — just the integer on stdout (script-friendly). Multiple inputs →
  a `wc`-like per-file breakdown plus `TOTAL`. Multiple models → an aligned
  comparison table. Both → a file × model matrix with a per-model basis legend.
  Approximations are marked inline: `~1428 (APPROXIMATE: o200k_base proxy)`.
- **`--json`** — a stable schema:
  `{ "results": [ { input, model, resolved_model, strategy, encoding?, basis, accuracy, approximation, tokens, stats? } ], "total": [...] | null }`.
  One result per (input × model); failed cells carry an `error` field instead of
  `tokens`. `total` is per-model and present only for multiple inputs. Each
  total has `model`, `tokens`, `complete`, `bases`, `accuracies`, and
  `approximations`. The three arrays contain distinct metadata from successful
  results in first-seen order. Mixed totals retain every contributing basis and
  accuracy; `approximations` lists each proxy encoding and reason, and is empty
  when no approximation contributed. Totals sum the separate input counts, so
  provider totals include an envelope for each successful input.
  `complete: false` means failed inputs were excluded. With no successful
  inputs, `tokens` is zero and the metadata arrays are empty; a successful
  zero-token count still contributes its metadata.
- **`-v/--verbose`** — a human breakdown: model, resolved target, strategy,
  basis, accuracy, approximation metadata, tokens.
- **`--stats`** — adds chars, words, bytes, and tokens-per-word. File × model
  matrices include a labeled `TOK/WORD` column for each model. Total ratios
  divide summed tokens by summed words for that model's successful inputs.
  Unavailable ratios are `—`, approximations have a `~` prefix, and partial
  column totals have a `*` suffix.

For example, a missing-key approximation over two files can produce this total:

```json
{
  "model": "claude-opus-4-8",
  "tokens": 411,
  "complete": true,
  "bases": ["raw-content"],
  "accuracies": ["approximate"],
  "approximations": [
    {"proxy_encoding": "o200k_base", "reason": "missing-api-key"}
  ]
}
```

A mixed total can contain both `content+envelope` and `raw-content` bases, or
multiple accuracy labels. Inspect those labels before comparing totals; they
describe different counting bases and levels of certainty.

## Files, globs & directories

```bash
toknt -m o200k_base a.txt b.txt        # per-file counts + TOTAL
toknt -m o200k_base "src/**/*.rs"      # quoted glob (toknt expands it)
toknt -m o200k_base src/               # recurse a directory
```

Directories recurse and **honor `.gitignore` by default**, skipping hidden
files. Override with `--no-ignore` (ignore no `.gitignore`) and `--hidden`
(include dotfiles). Unquoted shell globs are expanded by your shell; quoted
patterns are expanded by `toknt` and respect `--hidden`.

## Offline

`--offline` is a hard no-network contract: tiktoken still works, **cached**
open-weight tokenizers still work, while API-backed models and **uncached**
open-weight models error before any request — the latter with a `toknt pull`
hint. These offline errors remain errors when combined with `--approx`.

## Subcommands

```bash
toknt models [QUERY]          # list / search the known-model registry
toknt pull <MODEL>            # pre-fetch & cache a Hugging Face tokenizer
toknt cache ls                # list cached tokenizers
toknt cache rm <MODEL>        # remove one
toknt cache path              # print the cache directory
toknt completions <SHELL>     # bash | zsh | fish | powershell
```

### Shell completions

```bash
toknt completions bash > /usr/local/etc/bash_completion.d/toknt          # bash
toknt completions zsh  > "${fpath[1]}/_toknt"                            # zsh
toknt completions fish > ~/.config/fish/completions/toknt.fish           # fish
toknt completions powershell >> $PROFILE                                 # powershell
```

## Errors & exit codes

Errors are specific and actionable — they name the env var to set, the command
to run, or the input that was bad (no model, missing key, gated HF model,
missing file, invalid UTF-8, offline cache miss). Exit codes: **0** success,
**1** a runtime/count failure, **2** a CLI usage error.

## Environment

| Variable            | Purpose                                                         |
| ------------------- | --------------------------------------------------------------- |
| `TOKNT_MODEL`       | Default model when `-m` is omitted (flags take precedence).     |
| `TOKNT_CACHE_DIR`   | Override the tokenizer cache dir (else the platform XDG cache). |
| `ANTHROPIC_API_KEY` | Required for `claude*` models.                                  |
| `GEMINI_API_KEY`    | Required for `gemini*` models.                                  |
| `HF_TOKEN`          | For gated/private Hugging Face repos.                           |

## Development

```bash
cargo build              # debug build
cargo test               # correctness + CLI-surface tests (hermetic, offline)
cargo clippy --all-targets -- -D warnings
cargo fmt
cargo run --bin toknt-verify   # live verification -> verification.json (needs keys)
```

The counting **engine** is a reusable library (`src/lib.rs`), proven across all
four strategies by the committed `verification.json`. The **CLI**
(`src/main.rs`, `src/cli/`) is a thin product surface over it.

## CI and downloadable builds

[CI](https://github.com/reemus-dev/toknt/actions/workflows/ci.yml) runs on pull
requests, pushes to `main`, and version tags matching `v*`. It runs the existing
deterministic tests on Linux, macOS, and Windows. Linux also checks Rust
formatting, Clippy, and README/workflow formatting using the retained Node
tools. Rust comes from `rust-toolchain.toml`; Cargo uses the committed lockfile.
No provider API keys are needed, and routine CI preserves `verification.json`.
Run `toknt-verify` locally with your keys when you explicitly want live
evidence.

[Build binaries](https://github.com/reemus-dev/toknt/actions/workflows/build.yml)
runs on `v*` tags or manually from **Actions → Build binaries → Run workflow**.
Each native build uses `cargo build --locked --release --bin toknt`, then runs
the resulting CLI's help, version, and an exact offline count before upload.

| Platform          | Architecture  | Rust target                 | Runner              |
| ----------------- | ------------- | --------------------------- | ------------------- |
| Linux (GNU/glibc) | x64           | `x86_64-unknown-linux-gnu`  | Ubuntu 22.04        |
| Linux (GNU/glibc) | ARM64         | `aarch64-unknown-linux-gnu` | Ubuntu 22.04 ARM64  |
| macOS             | Apple Silicon | `aarch64-apple-darwin`      | macOS 15            |
| macOS             | Intel         | `x86_64-apple-darwin`       | macOS 15 Intel      |
| Windows           | x64           | `x86_64-pc-windows-msvc`    | Windows Server 2025 |
| Windows           | ARM64         | `aarch64-pc-windows-msvc`   | Windows 11 ARM64    |

These are standard GitHub-hosted runners. Cargo dependencies are cached; newer
CI runs cancel superseded runs for the same branch or pull request. Downloads
are retained for **14 days** under the completed build run's **Artifacts**. Each
`toknt-<target>` artifact contains a `.tar.gz` (`.zip` on Windows) with the
binary and README, plus a SHA-256 checksum. Unix archives retain executable
permissions. Extract the archive and put `toknt` (or `toknt.exe`) on your
`PATH`. Linux builds use glibc rather than musl; these are native build
artifacts, with no Developer ID signing or notarization configured for macOS.

To exercise builds without making a version tag:

```bash
gh workflow run build.yml --ref main
gh run list --workflow build.yml
gh run download RUN_ID --name toknt-aarch64-apple-darwin
```

For a versioned build, first update and commit `Cargo.toml` and `Cargo.lock` to
the intended version and wait for CI on `main` to pass. Then create and push the
matching tag (for example, `git tag v0.1.0` followed by
`git push origin v0.1.0`). Tags trigger both CI and binary builds; inspect both
runs before using the artifacts. Manual builds use the version already recorded
in Cargo. These workflows create Actions artifacts; a GitHub Release or package
registry publication is a separate step.
