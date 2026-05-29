# toknt — Goal 2: CLI Product

> **Second of two handoffs** (`/goal goal/task-2-cli.md`). Goal 1
> (`goal/task-1-core.md`) already built and **proved the counting engine** —
> `lib.rs` counts across all four strategies with explicit accuracy metadata,
> demonstrated by a populated `verification.json`. This goal builds the full
> `toknt` **CLI** on that proven core. The repo already holds real code and an
> updated `CLAUDE.md`: **read them first** and build on the existing library —
> do not reshape the core.

## Mission

Ship `toknt`: a fast, lightweight CLI that prints an **accurate token count**
for arbitrary UTF-8 text input — file(s), globs, directories, inline `-t`, or
piped stdin — against one or more **specific models**, with first-class
`--help`, shell completions, and a README. The counting itself is done; this
goal is the **product surface** around it: input routing, output formats,
subcommands, comparison, filesystem walking, stats, and UX.

## Counting basis (recap — drives output & comparison)

The core already implements this; what matters here is **displaying it
honestly**. Default basis is **raw content**. Offline strategies (tiktoken,
open-weight) report exact **content-only** tokens; API strategies (Anthropic,
Gemini) report a provider count/estimate over **content + a small fixed message
envelope** (inherent to their count-by-payload endpoints). Therefore:

- `toknt` never presents an approximation as exact, and never implies two counts
  on **different bases** are directly comparable.
- The library exposes each result's `basis` (`raw-content` |
  `content+envelope`), strategy, `accuracy` (`exact` | `provider-count` |
  `provider-estimate` | `approximate`), and approximation metadata — surface
  these in `--json` and `-v`, and use them to annotate comparison tables. (Full
  rationale: `goal/task-1-core.md` → _What a count means_.)

---

## Definition of Done

The run is complete when **all** of the following hold:

- [ ] `toknt -m <model> <input>` prints a token count for every strategy,
      reusing the Goal 1 library (no core rewrite).
- [ ] All required behaviors below are implemented and tested; no half-wired
      commands, stale help text, or broken tests.
- [ ] `--help` is excellent at every level; **shell completions** generate for
      bash, zsh, fish, and PowerShell; a **`README.md`** documents install
      (from-source), usage, and examples.
- [ ] `cargo build --release` is clean; `cargo clippy` clean;
      `cargo fmt --check` passes; the existing engine tests and explicit live
      verification command still pass.
- [ ] CLI-surface tests cover input routing, exit codes, error messages, the
      `--json` schema, comparison output, and per-file output.
- [ ] Manual CLI verification is run against the built binary across the command
      and flag matrix below; the final handoff summary reports what was run and
      whether each command passed.

**Out of scope** (unchanged): distribution/CI/packaging/publishing and cost
estimation. Build so they're trivial to add later (single static binary, clean
library/CLI split) — but do not build them now.

---

## Required Scope

- Thin `main.rs` over the existing library; clean `clap` (+ `clap_complete`)
  command tree.
- Input: file positionals, globs, directories, `-t/--text` inline, and **stdin
  auto-detected** (via `IsTerminal`). Multiple files/globs/directories produce a
  per-file breakdown and total.
- A model must resolve from `-m/--model` or `TOKNT_MODEL` unless `--approx` is
  used; helpful error when omitted. **`--approx`** escape hatch (covers both
  no-model and no-API-key: a clearly-labeled estimate via a tiktoken
  `o200k_base`/`cl100k_base` proxy). `--approx` allows fallback; it does not
  force approximation when a trusted exact/provider count is available.
- **Trusted-count-or-error** policy: exact offline counts and provider
  counts/estimates are allowed; approximations require explicit `--approx` and
  are always clearly labeled, never silent.
- Output: bare integer to stdout (default), `--json`, `-v/--verbose`.
- `--offline` (never touch the network: API models error, uncached open-weight
  errors with a `toknt pull` hint, tiktoken still works).
- Multi-model comparison: repeated `-m` produces an aligned table; each row
  labels its **basis** (API rows flag the included message envelope), never
  implying a like-for-like delta.
- Per-file breakdown: multiple files / globs / directories produce per-file
  counts plus `TOTAL` (`wc`-like). Directories recurse, honoring `.gitignore` by
  default; `--no-ignore` and `--hidden` override (use the `ignore` crate).
- Extra metrics (`--stats`): chars, words, bytes, tokens/word.
- Subcommands: `models`, `pull <MODEL>`, `cache <ls|rm|path>`,
  `completions <SH>`.
- Model selection precedence is **flags > env** (`TOKNT_MODEL`) unless
  `--approx` is used.
- If Goal 1 did not expose enough cache helpers for these subcommands, add a
  narrow cache-admin API to the core; do not rewrite counting/resolution.
- Clear, actionable errors at the CLI boundary.

---

## CLI contract

```
toknt [OPTIONS] [INPUTS]...

ARGS:
  [INPUTS]...        File paths, globs, or directories (omit when piping stdin)

CORE OPTIONS:
  -m, --model <ID>   Model/encoding to count for. Repeatable -> comparison.
  -t, --text <STR>   Count this inline string (instead of files/stdin).
      --approx       Accept a clearly-labeled approximation (see Trust policy).
      --offline      Never touch the network (no HF fetch, no API calls).

OUTPUT OPTIONS:
      --json         Machine-readable output (model, strategy, basis, accuracy,
                     approximation metadata, tokens, optional stats, per-file array).
  -v, --verbose      Human breakdown (model/strategy used, basis, accuracy).
      --stats        Include chars/words/bytes/tokens-per-word.

INPUT/FS OPTIONS (with file inputs):
      --no-ignore    Do not honor .gitignore when walking directories.
      --hidden       Include hidden files when walking directories.

SUBCOMMANDS:
  models             List / search the known-model registry.
  pull <MODEL>       Pre-fetch & cache an open-weight tokenizer for offline use.
  cache <ls|rm|path> Manage the tokenizer cache.
  completions <SH>   Print shell completions (bash|zsh|fish|powershell).
```

### Canonical examples

```bash
toknt -m o200k_base file.txt                    # exact, offline -> 1423
echo "hello world" | toknt -m o200k_base        # stdin -> 2
toknt -m gpt-5.2 -t "hello world"               # OpenAI model alias -> o200k_base
toknt -m deepseek-ai/DeepSeek-V4-Pro file.txt   # HF open-weight (auto-detected)
toknt -m claude-opus-4-8 file.txt               # Anthropic provider estimate (needs key)
toknt -m gpt-5.2 -m claude-opus-4-8 -m Qwen/Qwen3.5-9B file.txt  # comparison
toknt -m o200k_base src/*.rs docs/              # per-file breakdown + total
toknt -m o200k_base --stats file.txt            # include chars/words/bytes/tokens-per-word
toknt --approx file.txt                         # generic labeled estimate (no model)
toknt -m claude-opus-4-8 --approx f.txt         # labeled estimate when no API key
toknt file.txt                                  # ERROR: no model (guidance + --approx)
```

---

## Output

- **Default**: just the integer on stdout, nothing else (script-friendly). For
  multiple inputs → per-file breakdown + `TOTAL`. For multiple models →
  comparison table.
- **`--json`**: structured, stable schema (model, strategy, basis, accuracy,
  approximation metadata, tokens, optional stats, per-file array).
- **`-v/--verbose`**: human breakdown including model/strategy/basis and
  accuracy.
- Approximations clearly marked in every format (e.g.
  `~1428 (APPROXIMATE: cl100k proxy)`), never silently exact.
- Comparison tables label each row's **basis** and flag API rows that include
  the provider message envelope.

---

## Errors & UX

Specific and actionable:

- No model:
  `no model specified — pass -m <model>, set TOKNT_MODEL, or use --approx for an estimate. See: toknt models`.
- No key: `claude requires ANTHROPIC_API_KEY — set it, or run with --approx`.
- Gated model w/o token: hint `HF_TOKEN` + license acceptance.
- Missing file: `no such file: <path>` (never silently treat it as text).
- Invalid UTF-8 input: `input is not valid UTF-8: <path>` (never silently
  lossy-decode).
- Offline + uncached: hint `toknt pull <model>`.
- Unknown OpenAI model/encoding offline: say so; suggest `--approx`.

---

## Implementation guardrails

- **stdin:** auto-detect a pipe via `IsTerminal`. With no inputs, no `-t`, and
  no piped stdin → **error with guidance**, never block on a terminal read.
- **text decoding:** treat file/stdin input as UTF-8 text. Reject invalid UTF-8
  with an actionable error; do not silently replace bytes.
- **clap modeling:** subcommands take precedence over positionals; handle the
  rare clash with a file named like a subcommand (accept `./pull` or `--`).
- **HTTP/TLS:** the core already uses `reqwest` + **rustls** — keep it.
- Reuse the library's resolution/strategy/error types; the CLI only adds
  parsing, routing, formatting, and the subcommands.

---

## Tests (CLI surface)

Test the CLI, not just the core: input routing (file vs `-t` vs stdin), exit
codes, error messages, the `--json` schema, comparison output, and per-file
output. Keep the Goal 1 engine tests and live verification path green. Live API
checks stay key-gated (keys present here via mise; skip cleanly when absent).

Also manually execute the built CLI before calling the goal complete. Cover at
least:

- help: `toknt --help`, every subcommand `--help`, and generated completions for
  bash, zsh, fish, and PowerShell.
- input routing: `-t`, stdin, one file, missing file, invalid UTF-8 file.
- strategies: one tiktoken model/encoding, one HF model, Anthropic, Gemini.
- output modes: default integer, `--json`, `-v`, and `--stats`.
- trust controls: omitted model error, `--approx`, missing-key approximation if
  practical, `--offline` for tiktoken, API-backed model, cached HF, and uncached
  HF.
- cache commands: `pull`, `cache ls`, `cache path`, and `cache rm` using a
  temporary `TOKNT_CACHE_DIR` so the real cache is not damaged.
- full CLI surface: repeated `-m` comparison, multiple file inputs, glob input,
  recursive directory input, `.gitignore`, `--no-ignore`, `--hidden`, and
  `models` listing/search.

---

## References

- `clap` / `clap_complete`: https://docs.rs/clap
- `ignore` (gitignore-aware walking): https://docs.rs/ignore
- Counting core + basis rationale: [`goal/task-1-core.md`](task-1-core.md)
- Implementation quick-reference (crates, contracts, registry):
  [`goal/research.md`](research.md)

---

## Working agreement

- **Build on the proven core.** Before extending it, confirm Goal 1's
  `verification.json` is committed, the explicit live verification command has
  run, and `cargo test` is green — never build on an unproven library.
- Commit in logical, reviewable increments; branch off `main`.
- Sequence work in coherent increments — no half-wired feature on a green build.
- Keep `cargo clippy` and `cargo fmt` clean.
- When a fixed decision here proves wrong or impossible, **stop and surface
  it**.
- Match surrounding code idiom and density. Small, sharp tool over feature
  bloat.
