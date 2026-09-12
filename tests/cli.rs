//! CLI-surface tests: input routing, exit codes, error messages, the `--json`
//! schema, comparison output, and per-file output. Hermetic and offline —
//! tiktoken (compiled-in) and the approx proxy carry the happy paths, and the
//! `--offline` policy proves the API/HF error paths without any network. Live
//! API and real-HF behavior are proven separately by `verification.json`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::Value;
use toknt::{count, CountOptions};

// ---- harness --------------------------------------------------------------

/// A `toknt` invocation with host env scrubbed so tests are deterministic.
fn toknt() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_toknt"));
    cmd.env_remove("TOKNT_MODEL").env_remove("TOKNT_CACHE_DIR");
    cmd
}

struct Run {
    code: i32,
    out: String,
    err: String,
}

/// Run a command to completion. `stdin` is piped when `Some`, else `/dev/null`
/// (a non-terminal empty input — never a blocking terminal read).
fn run(cmd: &mut Command, stdin: Option<&[u8]>) -> Run {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = cmd.spawn().expect("spawn toknt");
    if let Some(bytes) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(bytes)
            .expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait toknt");
    Run {
        code: output.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&output.stdout).into_owned(),
        err: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh, unique temp directory for a test (cleaned up via [`TempDir`]).
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("toknt-cli-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn write(&self, name: &str, contents: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, contents).expect("write fixture");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn expected_tokens(text: &str, model: &str) -> usize {
    count(text, model, &CountOptions::default()).unwrap().tokens
}

// ---- input routing --------------------------------------------------------

#[test]
fn inline_text_prints_bare_integer() {
    let r = run(
        toknt().args(["-m", "o200k_base", "-t", "hello world"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out.trim(),
        expected_tokens("hello world", "o200k_base").to_string()
    );
    assert!(r.err.is_empty(), "no stderr on success: {:?}", r.err);
}

#[test]
fn stdin_is_counted_when_piped() {
    let r = run(toknt().args(["-m", "o200k_base"]), Some(b"hello world"));
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out.trim(),
        expected_tokens("hello world", "o200k_base").to_string()
    );
}

#[test]
fn empty_stdin_counts_as_zero() {
    let r = run(toknt().args(["-m", "o200k_base"]), Some(b""));
    assert_eq!(r.code, 0);
    assert_eq!(r.out.trim(), "0");
}

#[test]
fn a_single_file_prints_its_count() {
    let dir = TempDir::new();
    let file = dir.write("a.txt", b"the quick brown fox");
    let r = run(toknt().args(["-m", "o200k_base"]).arg(&file), None);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out.trim(),
        expected_tokens("the quick brown fox", "o200k_base").to_string()
    );
}

#[test]
fn text_flag_takes_precedence_over_stdin() {
    // -t wins; the piped stdin must be ignored entirely.
    let r = run(
        toknt().args(["-m", "o200k_base", "-t", "hello world"]),
        Some(b"this should be ignored entirely and would count differently"),
    );
    assert_eq!(
        r.out.trim(),
        expected_tokens("hello world", "o200k_base").to_string()
    );
}

// ---- errors & exit codes --------------------------------------------------

#[test]
fn missing_model_is_an_actionable_error() {
    let r = run(toknt().args(["-t", "hi"]), None);
    assert_eq!(r.code, 1);
    assert!(r.out.is_empty());
    assert!(r.err.contains("no model specified"), "stderr: {}", r.err);
    assert!(r.err.contains("--approx"));
}

#[test]
fn missing_file_is_reported_not_treated_as_text() {
    let r = run(
        toknt().args(["-m", "o200k_base", "/no/such/toknt/file.txt"]),
        None,
    );
    assert_eq!(r.code, 1);
    assert!(r.err.contains("no such file"), "stderr: {}", r.err);
}

#[test]
fn invalid_utf8_input_is_rejected() {
    let dir = TempDir::new();
    let file = dir.write("bad.bin", &[0x68, 0x69, 0xff]);
    let r = run(toknt().args(["-m", "o200k_base"]).arg(&file), None);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("not valid UTF-8"), "stderr: {}", r.err);
}

#[test]
fn unknown_model_without_approx_errors() {
    let r = run(
        toknt().args(["-m", "totally-unknown-thing", "-t", "hi"]),
        None,
    );
    assert_eq!(r.code, 1);
    assert!(r.err.contains("unknown model"), "stderr: {}", r.err);
}

// ---- approximation --------------------------------------------------------

#[test]
fn approx_with_no_model_is_marked_never_silent() {
    let r = run(toknt().args(["--approx", "-t", "hello world"]), None);
    assert_eq!(r.code, 0);
    assert!(r.out.starts_with('~'), "stdout: {}", r.out);
    assert!(r.out.contains("APPROXIMATE"), "stdout: {}", r.out);
}

#[test]
fn approx_does_not_force_estimate_when_exact_is_available() {
    // A resolvable tiktoken model with --approx must stay exact (bare integer).
    let r = run(
        toknt().args(["--approx", "-m", "o200k_base", "-t", "hello world"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out.trim(),
        expected_tokens("hello world", "o200k_base").to_string()
    );
    assert!(!r.out.contains("APPROXIMATE"));
}

#[test]
fn missing_key_errors_and_suggests_approx() {
    // Scrub the key so this is hermetic — the API key is checked before any
    // network call, so no request is made.
    let r = run(
        toknt()
            .env_remove("ANTHROPIC_API_KEY")
            .args(["-m", "claude-opus-4-8", "-t", "hi"]),
        None,
    );
    assert_eq!(r.code, 1);
    assert!(r.err.contains("ANTHROPIC_API_KEY"), "stderr: {}", r.err);
    assert!(r.err.contains("--approx"), "stderr: {}", r.err);
}

#[test]
fn missing_key_with_approx_falls_back_to_estimate() {
    let r = run(
        toknt().env_remove("ANTHROPIC_API_KEY").args([
            "--approx",
            "-m",
            "claude-opus-4-8",
            "-t",
            "hello world",
        ]),
        None,
    );
    assert_eq!(r.code, 0);
    assert!(r.out.contains("APPROXIMATE"), "stdout: {}", r.out);
}

// ---- output modes ---------------------------------------------------------

#[test]
fn json_single_result_schema() {
    let r = run(
        toknt().args(["-m", "o200k_base", "--json", "-t", "hello world"]),
        None,
    );
    assert_eq!(r.code, 0);
    let v: Value = serde_json::from_str(&r.out).expect("valid json");
    let res = &v["results"][0];
    assert_eq!(res["model"], "o200k_base");
    assert_eq!(res["strategy"], "tiktoken");
    assert_eq!(res["basis"], "raw-content");
    assert_eq!(res["accuracy"], "exact");
    assert_eq!(res["tokens"], expected_tokens("hello world", "o200k_base"));
    assert_eq!(res["input"], "(inline)");
    assert!(v["total"].is_null(), "single input -> null total");
}

#[test]
fn json_stats_block_present_with_flag() {
    let r = run(
        toknt().args(["-m", "o200k_base", "--json", "--stats", "-t", "hello world"]),
        None,
    );
    let v: Value = serde_json::from_str(&r.out).expect("valid json");
    let stats = &v["results"][0]["stats"];
    assert_eq!(stats["chars"], 11);
    assert_eq!(stats["words"], 2);
    assert_eq!(stats["bytes"], 11);
    assert!(stats["tokens_per_word"].is_number());
}

#[test]
fn json_per_file_total_sums_columns() {
    let dir = TempDir::new();
    let a = dir.write("a.txt", b"hello world");
    let b = dir.write("b.txt", b"the quick brown fox jumps");
    let r = run(
        toknt().args(["-m", "o200k_base", "--json"]).arg(&a).arg(&b),
        None,
    );
    assert_eq!(r.code, 0);
    let v: Value = serde_json::from_str(&r.out).expect("valid json");
    assert_eq!(v["results"].as_array().unwrap().len(), 2);
    let total = &v["total"][0];
    assert_eq!(total["model"], "o200k_base");
    assert_eq!(total["complete"], true);
    let expected = expected_tokens("hello world", "o200k_base")
        + expected_tokens("the quick brown fox jumps", "o200k_base");
    assert_eq!(total["tokens"], expected);
}

#[test]
fn verbose_shows_strategy_and_basis() {
    let r = run(
        toknt().args(["-m", "gpt-5.2", "-v", "-t", "hello world"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert!(r.out.contains("strategy:"));
    assert!(r.out.contains("tiktoken"));
    assert!(r.out.contains("basis:"));
    assert!(r.out.contains("resolved:"));
    assert!(r.out.contains("o200k_base"), "gpt-5.2 -> o200k_base");
}

#[test]
fn stats_flag_adds_tokens_per_word() {
    let r = run(
        toknt().args(["-m", "o200k_base", "--stats", "-t", "hello world"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert!(r.out.contains("tokens/word"), "stdout: {}", r.out);
    assert!(r.out.contains("chars 11"));
}

// ---- comparison & per-file ------------------------------------------------

#[test]
fn comparison_table_lists_each_model_with_basis() {
    let r = run(
        toknt().args(["-m", "o200k_base", "-m", "gpt-4", "-t", "hello world"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert!(r.out.contains("MODEL"));
    assert!(r.out.contains("BASIS"));
    assert!(r.out.contains("ACCURACY"));
    assert!(r.out.contains("o200k_base"));
    assert!(r.out.contains("gpt-4"));
}

#[test]
fn per_file_breakdown_has_total() {
    let dir = TempDir::new();
    let a = dir.write("a.txt", b"hello world");
    let b = dir.write("b.txt", b"the quick brown fox jumps");
    let r = run(toknt().args(["-m", "o200k_base"]).arg(&a).arg(&b), None);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("TOTAL"));
    assert!(r.out.contains("a.txt"));
    assert!(r.out.contains("b.txt"));
}

// ---- model precedence -----------------------------------------------------

#[test]
fn env_model_is_used_when_no_flag() {
    let r = run(
        toknt()
            .env("TOKNT_MODEL", "o200k_base")
            .args(["-t", "hello world"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out.trim(),
        expected_tokens("hello world", "o200k_base").to_string()
    );
}

#[test]
fn flag_beats_env_model() {
    let r = run(
        toknt()
            .env("TOKNT_MODEL", "gpt-4")
            .args(["-m", "o200k_base", "--json", "-t", "x"]),
        None,
    );
    let v: Value = serde_json::from_str(&r.out).expect("valid json");
    assert_eq!(v["results"][0]["resolved_model"], "o200k_base");
}

// ---- offline policy (no network) ------------------------------------------

#[test]
fn offline_tiktoken_still_counts() {
    let r = run(
        toknt().args(["-m", "o200k_base", "--offline", "-t", "hello world"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out.trim(),
        expected_tokens("hello world", "o200k_base").to_string()
    );
}

#[test]
fn offline_blocks_api_model() {
    let r = run(
        toknt().args(["-m", "claude-opus-4-8", "--offline", "-t", "hi"]),
        None,
    );
    assert_eq!(r.code, 1);
    assert!(r.err.contains("offline"), "stderr: {}", r.err);
    assert!(r.err.contains("anthropic"));
}

#[test]
fn offline_uncached_open_weight_hints_pull() {
    let dir = TempDir::new();
    let r = run(
        toknt().env("TOKNT_CACHE_DIR", dir.path()).args([
            "-m",
            "some-org/not-cached",
            "--offline",
            "-t",
            "hi",
        ]),
        None,
    );
    assert_eq!(r.code, 1);
    assert!(r.err.contains("toknt pull"), "stderr: {}", r.err);
}

#[test]
fn offline_api_model_still_errors_with_approx() {
    let r = run(
        toknt().args(["-m", "claude-opus-4-8", "--offline", "--approx", "-t", "hi"]),
        None,
    );
    assert_eq!(r.code, 1);
    assert!(r.out.is_empty(), "stdout: {}", r.out);
    assert!(r.err.contains("offline"), "stderr: {}", r.err);
    assert!(r.err.contains("anthropic"), "stderr: {}", r.err);
}

#[test]
fn offline_uncached_open_weight_still_errors_with_approx() {
    let dir = TempDir::new();
    let r = run(
        toknt().env("TOKNT_CACHE_DIR", dir.path()).args([
            "-m",
            "some-org/not-cached",
            "--offline",
            "--approx",
            "-t",
            "hi",
        ]),
        None,
    );
    assert_eq!(r.code, 1);
    assert!(r.out.is_empty(), "stdout: {}", r.out);
    assert!(r.err.contains("toknt pull"), "stderr: {}", r.err);
}

// ---- directory walking ----------------------------------------------------

#[test]
fn directory_walk_honors_gitignore_by_default() {
    let dir = TempDir::new();
    // Two surviving files so the per-file breakdown names them (a single
    // surviving file would collapse to the bare-integer scalar form).
    dir.write("a.txt", b"alpha alpha alpha");
    dir.write("b.txt", b"beta beta beta");
    dir.write("ignored.txt", b"should not appear in output anywhere");
    dir.write(".gitignore", b"ignored.txt\n");
    let r = run(toknt().args(["-m", "o200k_base"]).arg(dir.path()), None);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("a.txt"));
    assert!(r.out.contains("b.txt"));
    assert!(
        !r.out.contains("ignored.txt"),
        "gitignored file leaked: {}",
        r.out
    );
}

#[test]
fn no_ignore_includes_gitignored_files() {
    let dir = TempDir::new();
    dir.write("a.txt", b"alpha");
    dir.write("ignored.txt", b"beta");
    dir.write(".gitignore", b"ignored.txt\n");
    let r = run(
        toknt()
            .args(["-m", "o200k_base", "--no-ignore"])
            .arg(dir.path()),
        None,
    );
    assert_eq!(r.code, 0);
    assert!(r.out.contains("ignored.txt"), "stdout: {}", r.out);
}

#[test]
fn glob_pattern_expands_to_matching_files() {
    let dir = TempDir::new();
    dir.write("a.txt", b"alpha");
    dir.write("b.txt", b"beta");
    dir.write("c.md", b"gamma");
    let pattern = format!("{}/*.txt", dir.path().display());
    let r = run(toknt().args(["-m", "o200k_base"]).arg(&pattern), None);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("a.txt"));
    assert!(r.out.contains("b.txt"));
    assert!(
        !r.out.contains("c.md"),
        "non-matching file included: {}",
        r.out
    );
    assert!(r.out.contains("TOTAL"));
}

// ---- subcommands ----------------------------------------------------------

#[test]
fn models_lists_registry() {
    let r = run(toknt().arg("models"), None);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("o200k_base"));
    assert!(r.out.contains("claude*"));
    assert!(r.out.contains("org/name"));
}

#[test]
fn models_search_filters() {
    let r = run(toknt().args(["models", "claude"]), None);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("claude*"));
    assert!(!r.out.contains("o200k_harmony"), "filter leaked: {}", r.out);
}

#[test]
fn cache_path_prints_resolved_dir() {
    let dir = TempDir::new();
    let r = run(
        toknt()
            .env("TOKNT_CACHE_DIR", dir.path())
            .args(["cache", "path"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert_eq!(r.out.trim(), dir.path().to_string_lossy());
}

#[test]
fn cache_ls_empty_reports_empty() {
    let dir = TempDir::new();
    let r = run(
        toknt()
            .env("TOKNT_CACHE_DIR", dir.path())
            .args(["cache", "ls"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert!(r.out.contains("empty"), "stdout: {}", r.out);
}

#[test]
fn cache_rm_absent_is_idempotent() {
    let dir = TempDir::new();
    let r = run(
        toknt()
            .env("TOKNT_CACHE_DIR", dir.path())
            .args(["cache", "rm", "org/never-cached"]),
        None,
    );
    assert_eq!(r.code, 0);
    assert!(r.out.contains("not cached"), "stdout: {}", r.out);
}

#[test]
fn pull_rejects_non_open_weight_models() {
    let r = run(toknt().args(["pull", "o200k_base"]), None);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("open-weight"), "stderr: {}", r.err);
}

#[test]
fn completions_generate_for_all_shells() {
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let r = run(toknt().args(["completions", shell]), None);
        assert_eq!(r.code, 0, "{shell} completions exit");
        assert!(!r.out.trim().is_empty(), "{shell} produced no output");
        assert!(r.out.contains("toknt"), "{shell} script names the binary");
    }
}

// ---- clap usage errors ----------------------------------------------------

#[test]
fn text_and_positional_inputs_conflict() {
    let r = run(
        toknt().args(["-m", "o200k_base", "-t", "x", "somefile.txt"]),
        None,
    );
    assert_eq!(r.code, 2, "clap usage error exits 2");
}
