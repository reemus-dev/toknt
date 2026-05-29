//! Live verification harness — the Goal 1 gate.
//!
//! Drives the `toknt` core over all ten proof targets on the shared sample,
//! writes/refreshes `verification.json` at the repo root, and asserts every
//! target produced a count. API/HF targets skip cleanly only when their
//! credential is genuinely absent; with keys present (as in this repo) they
//! must populate, and any failure is fatal.
//!
//! Run via mise so `.env` keys load, e.g. `mise exec -- cargo run --bin toknt-verify`.

use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest, Sha256};

use toknt::{count_bytes, CountOptions, CountResult, TokntError};

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR");
const SAMPLE_REL: &str = "tests/fixtures/samples/sample.txt";

/// The ten proof targets — one per row of `verification.json`. Together they
/// cover every strategy (tiktoken / anthropic / gemini / open-weight).
const PROOF_TARGETS: &[&str] = &[
    "o200k_base",
    "claude-opus-4-8",
    "gemini-3.5-flash",
    "google/gemma-4-31B-it",
    "XiaomiMiMo/MiMo-V2.5",
    "XiaomiMiMo/MiMo-V2.5-Pro",
    "deepseek-ai/DeepSeek-V4-Flash",
    "deepseek-ai/DeepSeek-V4-Pro",
    "Qwen/Qwen3.5-27B",
    "Qwen/Qwen3.5-9B",
];

#[derive(Serialize)]
struct Verification {
    generated_at: String,
    sample: SampleInfo,
    results: Vec<CountResult>,
}

#[derive(Serialize)]
struct SampleInfo {
    path: String,
    bytes: usize,
    sha256: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("verification failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let sample_path = Path::new(REPO_ROOT).join(SAMPLE_REL);
    let bytes = std::fs::read(&sample_path)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", sample_path.display()))?;

    let sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        format!("{:x}", hasher.finalize())
    };
    println!(
        "sample: {SAMPLE_REL} ({} bytes, sha256 {}…)",
        bytes.len(),
        &sha256[..12]
    );

    let options = CountOptions::default(); // allow-network, exact-only
    let mut results = Vec::new();
    let mut skipped: Vec<(&str, String)> = Vec::new();
    let mut failures: Vec<(&str, TokntError)> = Vec::new();

    for &model in PROOF_TARGETS {
        match count_bytes(&bytes, SAMPLE_REL, model, &options) {
            Ok(result) => {
                println!(
                    "  ✓ {model:<32} {:>6} tokens  [{}/{}]",
                    result.tokens,
                    strategy_label(&result),
                    accuracy_label(&result),
                );
                results.push(result);
            }
            Err(err) if is_clean_skip(&err) => {
                println!("  · {model:<32} skipped (credential absent)");
                skipped.push((model, err.to_string()));
            }
            Err(err) => {
                eprintln!("  ✗ {model:<32} {err}");
                failures.push((model, err));
            }
        }
    }

    if !failures.is_empty() {
        anyhow::bail!(
            "{} proof target(s) failed with credentials present; refusing to write a partial \
             artifact",
            failures.len()
        );
    }

    let verification = Verification {
        generated_at: today_utc(),
        sample: SampleInfo {
            path: SAMPLE_REL.to_string(),
            bytes: bytes.len(),
            sha256,
        },
        results,
    };

    let out_path = Path::new(REPO_ROOT).join("verification.json");
    let mut json = serde_json::to_string_pretty(&verification)?;
    json.push('\n');
    std::fs::write(&out_path, json)?;

    println!(
        "\nwrote {} ({} populated, {} skipped)",
        out_path.display(),
        verification.results.len(),
        skipped.len()
    );

    // In this repo every key is present, so all ten must populate.
    if skipped.is_empty() && verification.results.len() == PROOF_TARGETS.len() {
        println!("all {} proof targets populated ✓", PROOF_TARGETS.len());
    } else if !skipped.is_empty() {
        println!(
            "note: {} target(s) skipped because a credential was absent (clean skip)",
            skipped.len()
        );
    }
    Ok(())
}

/// A failure counts as a clean skip only when the credential is genuinely
/// missing — `MissingApiKey` is produced only in that case, and HF auth errors
/// count as a skip only when `HF_TOKEN` is unset.
fn is_clean_skip(err: &TokntError) -> bool {
    match err {
        TokntError::MissingApiKey { .. } => true,
        TokntError::HfGated { .. } | TokntError::HfDownload { .. } => std::env::var("HF_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())
            .is_none(),
        _ => false,
    }
}

fn strategy_label(result: &CountResult) -> String {
    serde_json::to_value(result.strategy)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "?".to_string())
}

fn accuracy_label(result: &CountResult) -> String {
    serde_json::to_value(result.accuracy)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "?".to_string())
}

/// Current UTC date as `YYYY-MM-DD` (Howard Hinnant's civil-from-days), no deps.
fn today_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}")
}
