# toknt — Goals (handoff index)

`toknt` is built in **two sequential autonomous-agent handoffs**. Each goal doc
below is a self-contained source of truth for its run; start the matching
session with `/goal goal/<file>`.

Both goals were verified on 2026-09-12. See the committed
[`verification.json`](../verification.json) for core proof and the
[Goal 2 verification record](goal-2-verification.md) for CLI gates and manual
coverage.

## 1. [`goal/task-1-core.md`](task-1-core.md) — Counting engine

Build the accurate counting **core** as a Rust library and prove it across all
four strategies (tiktoken, open-weight, Anthropic, Gemini). **Pass point:** a
committed `verification.json`, produced by an explicit live verification
command, populated with the token count and accuracy metadata each proof target
produced for a shared sample, plus correctness guards and a clean
`build`/`clippy`/`fmt`. Includes the TypeScript→Rust repo migration. No
user-facing CLI yet.

## 2. [`goal/task-2-cli.md`](task-2-cli.md) — CLI product

Build the full `toknt` **CLI** on the proven core: input routing (files/`-t`/
stdin), output (`--json`/`-v`/`--stats`), subcommands (`models`/`pull`/`cache`/
`completions`), multi-model comparison, per-file/glob/directory walking,
`--help`, shell completions, manual CLI verification, and a README.

---

Run them in order — Goal 2 assumes Goal 1's library and `verification.json`
exist. Implementation quick-reference (pinned crates, provider HTTP contracts,
model→strategy registry, cache decisions): [`goal/research.md`](research.md).
