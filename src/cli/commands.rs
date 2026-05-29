//! Subcommand handlers. These reuse the library's registry, resolution, and
//! cache-admin helpers verbatim — the CLI only lists, prefetches, and formats.

use std::io;

use anyhow::bail;
use clap::CommandFactory;
use toknt::{CountOptions, Strategy, Target};

use super::{CacheAction, Cli, ShellArg};

/// `toknt models [QUERY]` — list (or case-insensitively filter) the registry.
pub fn models(query: Option<&str>) -> anyhow::Result<u8> {
    let needle = query.map(str::to_lowercase);
    let all = toknt::known_models();
    let rows: Vec<_> = all
        .iter()
        .filter(|m| match &needle {
            Some(q) => {
                m.pattern.to_lowercase().contains(q)
                    || m.detail.to_lowercase().contains(q)
                    || strategy_label(m.strategy).contains(q.as_str())
            }
            None => true,
        })
        .collect();

    if rows.is_empty() {
        println!("no known model matches {:?}", query.unwrap_or(""));
        return Ok(0);
    }

    let pw = rows
        .iter()
        .map(|m| m.pattern.len())
        .max()
        .unwrap_or(0)
        .max("PATTERN".len());
    let sw = rows
        .iter()
        .map(|m| strategy_label(m.strategy).len())
        .max()
        .unwrap_or(0)
        .max("STRATEGY".len());

    println!("{:<pw$}  {:<sw$}  DETAIL", "PATTERN", "STRATEGY");
    for m in rows {
        println!(
            "{:<pw$}  {:<sw$}  {}",
            m.pattern,
            strategy_label(m.strategy),
            m.detail
        );
    }
    println!(
        "\nPrefix any id with openai:/anthropic:/google:/hf: to force a strategy. \
         Patterns ending in * match a family; org/name is any Hugging Face repo."
    );
    Ok(0)
}

/// `toknt pull <MODEL>` — prefetch an open-weight tokenizer into the cache.
pub fn pull(model: &str) -> anyhow::Result<u8> {
    let opts = CountOptions::default();
    let repo = match toknt::resolve(model)? {
        Target::OpenWeight { repo } => repo,
        other => bail!(
            "pull only applies to open-weight (Hugging Face) models; {model:?} resolves to the \
             {} strategy, which needs no tokenizer cache.",
            strategy_label(other.strategy())
        ),
    };
    let path = toknt::pull(&repo, &opts)?;
    println!("pulled {repo}");
    println!("  cached at {}", path.display());
    Ok(0)
}

/// `toknt cache <ls|rm|path>` — manage the toknt-owned tokenizer cache.
pub fn cache(action: CacheAction) -> anyhow::Result<u8> {
    let opts = CountOptions::default();
    match action {
        CacheAction::Ls => {
            let entries = toknt::list_cached(&opts)?;
            if entries.is_empty() {
                println!("cache is empty ({})", toknt::cache_dir(&opts)?.display());
                return Ok(0);
            }
            let w = entries
                .iter()
                .map(|e| e.repo.len())
                .max()
                .unwrap_or(0)
                .max("REPO".len());
            println!("{:<w$}  PATH", "REPO");
            for e in entries {
                println!("{:<w$}  {}", e.repo, e.path.display());
            }
            Ok(0)
        }
        CacheAction::Rm { model } => {
            let repo = model.strip_prefix("hf:").unwrap_or(&model);
            if toknt::remove_cached(repo, &opts)? {
                println!("removed {repo} from cache");
            } else {
                println!("not cached: {repo}");
            }
            Ok(0)
        }
        CacheAction::Path => {
            println!("{}", toknt::cache_dir(&opts)?.display());
            Ok(0)
        }
    }
}

/// `toknt completions <SHELL>` — print a completion script to stdout.
pub fn completions(shell: ShellArg) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(
        clap_complete::Shell::from(shell),
        &mut cmd,
        name,
        &mut io::stdout(),
    );
}

fn strategy_label(strategy: Strategy) -> &'static str {
    match strategy {
        Strategy::Tiktoken => "tiktoken",
        Strategy::OpenWeight => "open-weight",
        Strategy::Anthropic => "anthropic",
        Strategy::Gemini => "gemini",
        Strategy::Approx => "approx",
    }
}
