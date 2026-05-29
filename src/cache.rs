//! Tokenizer cache & network policy for the open-weight path.
//!
//! Open-weight `tokenizer.json` files fetch once from the HF Hub into a
//! toknt-owned cache dir, then count offline. The `offline` policy is a hard
//! no-network contract: it checks the cache and errors *before* any client is
//! built. Goal 2 drives the public `pull` / `cached_path` / `list_cached` /
//! `remove_cached` helpers from its `pull`/`cache` subcommands.

use std::path::{Path, PathBuf};

use etcetera::base_strategy::{choose_base_strategy, BaseStrategy};
use hf_hub::api::sync::ApiBuilder;
use hf_hub::Cache;

use crate::error::TokntError;
use crate::options::{CountOptions, NetworkPolicy};

const TOKENIZER_FILE: &str = "tokenizer.json";
const TOKENIZER_CONFIG_FILE: &str = "tokenizer_config.json";

/// A cached open-weight tokenizer entry.
#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub repo: String,
    pub path: PathBuf,
}

/// Resolve the toknt-owned cache dir: explicit override → `TOKNT_CACHE_DIR` →
/// platform cache dir (`<cache>/toknt`).
pub fn cache_dir(opts: &CountOptions) -> Result<PathBuf, TokntError> {
    if let Some(dir) = &opts.cache_dir {
        return Ok(dir.clone());
    }
    if let Ok(dir) = std::env::var("TOKNT_CACHE_DIR") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    let strategy = choose_base_strategy().map_err(|e| TokntError::CacheDir {
        detail: e.to_string(),
    })?;
    Ok(strategy.cache_dir().join("toknt"))
}

fn hf_token(opts: &CountOptions) -> Option<String> {
    opts.hf_token
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HF_TOKEN").ok().filter(|s| !s.is_empty()))
}

/// The HF on-disk cache folder name for a repo: `models--<org>--<name>`.
fn folder_name(repo: &str) -> String {
    format!("models--{}", repo.replace('/', "--"))
}

/// Inverse of [`folder_name`] for listing (best effort: a literal `--` in a
/// repo name is indistinguishable from the `/` separator).
fn decode_folder_name(folder: &str) -> Option<String> {
    folder
        .strip_prefix("models--")
        .map(|rest| rest.replace("--", "/"))
}

/// Extract the resolved revision from a cached path of the form
/// `.../snapshots/<sha>/tokenizer.json`.
fn revision_from_path(path: &Path) -> Option<String> {
    let comps: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let i = comps.iter().position(|c| c == "snapshots")?;
    comps.get(i + 1).cloned()
}

fn classify_hf_error(repo: &str, err: hf_hub::api::sync::ApiError) -> TokntError {
    let detail = err.to_string();
    let low = detail.to_lowercase();
    let looks_gated = low.contains("401")
        || low.contains("403")
        || low.contains("unauthorized")
        || low.contains("forbidden")
        || low.contains("authentication")
        || low.contains("gated");
    if looks_gated {
        TokntError::HfGated {
            model: repo.to_string(),
        }
    } else {
        TokntError::HfDownload {
            model: repo.to_string(),
            detail,
        }
    }
}

/// Ensure `tokenizer.json` for `repo` is available, returning `(path, revision)`.
/// Under [`NetworkPolicy::Offline`] this never touches the network: a cache hit
/// succeeds, a miss returns [`TokntError::OfflineCacheMiss`].
pub(crate) fn ensure_tokenizer(
    repo: &str,
    opts: &CountOptions,
) -> Result<(PathBuf, String), TokntError> {
    let dir = cache_dir(opts)?;

    if opts.network == NetworkPolicy::Offline {
        return match Cache::new(dir).model(repo.to_string()).get(TOKENIZER_FILE) {
            Some(path) => {
                let revision = revision_from_path(&path).unwrap_or_else(|| "main".to_string());
                Ok((path, revision))
            }
            None => Err(TokntError::OfflineCacheMiss {
                model: repo.to_string(),
            }),
        };
    }

    let api = ApiBuilder::new()
        .with_cache_dir(dir)
        .with_token(hf_token(opts))
        .with_progress(false)
        .build()
        .map_err(|e| classify_hf_error(repo, e))?;
    let model = api.model(repo.to_string());
    let path = model
        .get(TOKENIZER_FILE)
        .map_err(|e| classify_hf_error(repo, e))?;
    // Sidecar config is useful future metadata, but raw counting only needs
    // tokenizer.json; many repos omit the sidecar, so cache it opportunistically.
    let _ = model.get(TOKENIZER_CONFIG_FILE);
    let revision = revision_from_path(&path).unwrap_or_else(|| "main".to_string());
    Ok((path, revision))
}

/// Prefetch a tokenizer into the cache (always networked); returns its path.
/// Backs Goal 2's `toknt pull <model>`.
pub fn pull(repo: &str, opts: &CountOptions) -> Result<PathBuf, TokntError> {
    let dir = cache_dir(opts)?;
    let api = ApiBuilder::new()
        .with_cache_dir(dir)
        .with_token(hf_token(opts))
        .with_progress(false)
        .build()
        .map_err(|e| classify_hf_error(repo, e))?;
    let model = api.model(repo.to_string());
    let path = model
        .get(TOKENIZER_FILE)
        .map_err(|e| classify_hf_error(repo, e))?;
    // Keep `pull` aligned with normal counting: tokenizer.json is required,
    // tokenizer_config.json is cached when present.
    let _ = model.get(TOKENIZER_CONFIG_FILE);
    Ok(path)
}

/// The cached tokenizer path for `repo` if present, without any network.
pub fn cached_path(repo: &str, opts: &CountOptions) -> Result<Option<PathBuf>, TokntError> {
    let dir = cache_dir(opts)?;
    Ok(Cache::new(dir).model(repo.to_string()).get(TOKENIZER_FILE))
}

/// List cached open-weight tokenizer entries.
pub fn list_cached(opts: &CountOptions) -> Result<Vec<CachedEntry>, TokntError> {
    let dir = cache_dir(opts)?;
    let mut entries = Vec::new();
    let read = match std::fs::read_dir(&dir) {
        Ok(read) => read,
        Err(_) => return Ok(entries), // no cache dir yet → nothing cached
    };
    for dir_entry in read.flatten() {
        let folder = dir_entry.file_name().to_string_lossy().into_owned();
        let Some(repo) = decode_folder_name(&folder) else {
            continue;
        };
        if let Some(path) = Cache::new(dir.clone())
            .model(repo.clone())
            .get(TOKENIZER_FILE)
        {
            entries.push(CachedEntry { repo, path });
        }
    }
    entries.sort_by(|a, b| a.repo.cmp(&b.repo));
    Ok(entries)
}

/// Remove a cached tokenizer entry; returns whether anything was removed.
pub fn remove_cached(repo: &str, opts: &CountOptions) -> Result<bool, TokntError> {
    let dir = cache_dir(opts)?;
    let folder = dir.join(folder_name(repo));
    if folder.exists() {
        std::fs::remove_dir_all(&folder).map_err(|e| TokntError::CacheDir {
            detail: e.to_string(),
        })?;
        Ok(true)
    } else {
        Ok(false)
    }
}
