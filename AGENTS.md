# CLAUDE.md

## What this is

`toknt` — a fast, lightweight CLI that reports an **accurate token count** for
arbitrary input (file, inline `-t`, or piped stdin) against a **specific
model**. Exact & offline for OpenAI (tiktoken) and open-weight HF models;
provider counts/estimates via live API for Claude and Gemini.

**The work is split into two sequential autonomous-agent handoffs — those are
the source of truth:**

1. [`goal/task-1-core.md`](./goal/task-1-core.md) — Goal 1: the counting
   **engine** (library + all four strategies), proven by a committed
   `verification.json`.
2. [`goal/task-2-cli.md`](./goal/task-2-cli.md) — Goal 2: the full **CLI** on
   the proven core.

Read the relevant goal before making product decisions;
[`goal/index.md`](./goal/index.md) is a one-page index of both. Implementation
quick-reference (pinned crates, provider HTTP contracts, model→strategy
registry, cache decisions): [`goal/research.md`](./goal/research.md).

## Stack

Rust, single self-contained binary named `toknt`. Core crates: `clap` (+
`clap_complete`) for the CLI, `tiktoken-rs` (OpenAI), `tokenizers` + `hf-hub`
(open-weight), `reqwest` + **rustls** (Anthropic/Gemini endpoints). Keep the
binary lean — prefer raw HTTP over heavy provider SDKs; do not embed open-weight
tokenizers (`tiktoken-rs`'s compiled-in BPE ranks are expected and fine).

> The repo carries a TypeScript/Node toolchain (prettier/eslint, `package.json`,
> `node_modules/`, …) **alongside** the Rust project — it stays, backing the
> formatting setup and hooks on the repo's Markdown/configs. The shipped `toknt`
> is a self-contained Rust binary; the Node files are dev-tooling only. See the
> migration section of `goal/task-1-core.md`.

## Architecture

- **Library/CLI split**: counting core in a library (`lib.rs`), thin CLI in
  `main.rs`. The core is unit-testable without the CLI. (Goal 1 builds the
  library; Goal 2 builds the CLI on top.)
- **Strategy routing**: a resolved model maps to one of `tiktoken` /
  `open-weight` / `anthropic` / `gemini` / `approx`. See `goal/task-1-core.md` →
  _Model resolution_ and _Provider strategies_.
- **Counting basis**: default is **raw content** (offline = content-only; API =
  content + a small message envelope). Surfaced per result, never conflated. See
  `goal/task-1-core.md` → _What a count means_.
- **Cache**: open-weight `tokenizer.json` files download on demand to a
  toknt-owned XDG cache (override `TOKNT_CACHE_DIR`).

## Commands

```bash
cargo build            # debug build
cargo build --release  # release binary -> target/release/toknt
cargo run -- <args>    # run the CLI
cargo test             # correctness tests; do not rewrite verification.json
cargo clippy           # lint — keep clean
cargo fmt              # format — keep clean
```

- Toolchain is managed by **mise** (`.mise.toml`); add the Rust toolchain there.
- `Taskfile.yaml` should wrap the above (`task build`, `task test`, …).
- **API keys**: mise auto-loads `.env`, which provides `ANTHROPIC_API_KEY`,
  `GEMINI_API_KEY`, `OPENAI_API_KEY`, `HF_TOKEN` — and exports them to child
  processes, so live verification and real counts work in this repo (run
  commands via mise so keys reach the process). Live checks must still skip
  cleanly when keys are absent.
- Token count of any content: `aut tokens count <path>` or pipe to it.

## Conventions

- **Accuracy is the product.** Offline tokenizer counts are exact-or-error; API
  counts are labeled provider counts/estimates; approximation happens only via
  explicit `--approx` and is always clearly labeled — never silently presented
  as exact.
- **Model is required** (no silent default); `--approx` is the universal "I
  accept an estimate" escape hatch (covers no-model and no-API-key cases).
- Default count is **raw content**; the faithful `--chat` mode is a later phase.
- Errors are specific and actionable (tell the user the exact fix).
- Match surrounding code idiom and density. Small, sharp tool > feature bloat.
- `git mv`/`git rm` for tracked files; branch off `main` for large changes;
  commit in logical, reviewable increments.

## Working with autonomous goals

The goal files are executed as autonomous runs. When executing one:

- **Done means proven, not asserted.** A goal is complete only when its
  Definition of Done is _run and shown_ — execute `cargo test` and the explicit
  live verification command from `goal/task-1-core.md` (via mise, so `.env` keys
  load), then confirm the committed `verification.json` is fully populated.
  Never end a run on a self-attested checklist.
- **Commit at proven milestones**, not just at the end — after the Node→Rust
  migration, after each provider strategy passes its verification row, after the
  harness goes green. Each commit is a clippy/fmt-clean recovery point if a
  later step derails.
- **Delegate where it pays; keep the through-line in-thread.** Send read-heavy
  or parallel work to subagents (researching the Anthropic/Gemini count
  endpoints, surveying leftover boilerplate during migration, an independent
  verification pass), and hand a well-scoped slice with a fixed done-when to a
  dedicated implementer agent. Keep connective implementation — anything needing
  cross-cutting context — on the main thread so the through-line stays
  continuous.
- **Stop and surface** when a fixed decision in a goal file proves wrong — don't
  silently work around it.

## Boundaries (this phase)

Out of scope until a later phase: distribution/CI/packaging/publishing
(`cargo-dist`, Homebrew, curl installer, crates.io, npm) and cost estimation.
Build so they're trivial to add later (single static binary), but don't build
them now.
