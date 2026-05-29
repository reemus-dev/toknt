//! Input routing: turn the positional args + `-t` + stdin into an ordered list
//! of named byte buffers to count. Each [`Input`] keeps its display label (a
//! path, `(stdin)`, or `(inline)`) so per-file output and UTF-8 errors can name
//! their source.

use std::io::{IsTerminal, Read};
use std::path::Path;

use anyhow::{anyhow, bail, Context};
use ignore::WalkBuilder;

/// A single named input buffer. UTF-8 validation is deferred to the counting
/// layer so the library owns the one canonical invalid-UTF-8 error.
pub struct Input {
    /// Human-facing source label: a path, `(stdin)`, or `(inline)`.
    pub label: String,
    pub bytes: Vec<u8>,
}

/// How directory walking should treat ignore files and hidden entries.
#[derive(Clone, Copy)]
pub struct WalkOpts {
    pub no_ignore: bool,
    pub hidden: bool,
}

/// Resolve all inputs in priority order: inline `-t` wins; else the positional
/// paths/globs/dirs; else piped stdin. With none of those (a bare terminal),
/// error with guidance rather than blocking on a terminal read.
pub fn gather(inputs: &[String], text: Option<&str>, walk: WalkOpts) -> anyhow::Result<Vec<Input>> {
    if let Some(text) = text {
        return Ok(vec![Input {
            label: "(inline)".to_string(),
            bytes: text.as_bytes().to_vec(),
        }]);
    }

    if !inputs.is_empty() {
        let mut collected = Vec::new();
        for raw in inputs {
            expand_one(raw, walk, &mut collected)?;
        }
        if collected.is_empty() {
            bail!("no input files matched {inputs:?}");
        }
        return Ok(collected);
    }

    // No -t and no positionals: read stdin only if it is actually piped.
    if std::io::stdin().is_terminal() {
        bail!("no input — pass file paths, -t <text>, or pipe stdin. See: toknt --help");
    }
    let mut bytes = Vec::new();
    std::io::stdin()
        .read_to_end(&mut bytes)
        .context("reading stdin")?;
    Ok(vec![Input {
        label: "(stdin)".to_string(),
        bytes,
    }])
}

/// Expand a single positional into one or more inputs: an existing file is read
/// directly, a directory is walked, an unmatched path with glob metacharacters
/// is expanded, and anything else is a hard `no such file` error.
fn expand_one(raw: &str, walk: WalkOpts, out: &mut Vec<Input>) -> anyhow::Result<()> {
    let path = Path::new(raw);
    if path.is_dir() {
        return walk_dir(raw, walk, out);
    }
    if path.is_file() {
        out.push(read_file(path)?);
        return Ok(());
    }
    if has_glob_meta(raw) {
        return expand_glob(raw, walk, out);
    }
    bail!("no such file: {raw}");
}

/// Walk a directory, honoring `.gitignore` and skipping hidden files by default
/// (overridable via `--no-ignore` / `--hidden`). Files are emitted in a stable
/// sorted order so per-file output is deterministic.
fn walk_dir(dir: &str, walk: WalkOpts, out: &mut Vec<Input>) -> anyhow::Result<()> {
    let honor_ignore = !walk.no_ignore;
    let mut builder = WalkBuilder::new(dir);
    builder
        .hidden(!walk.hidden)
        .git_ignore(honor_ignore)
        .git_global(honor_ignore)
        .git_exclude(honor_ignore)
        .ignore(honor_ignore)
        .parents(honor_ignore)
        // Honor .gitignore even outside a git repo (the crate gates git rules on
        // a repo by default); --no-ignore turns every ignore source off above.
        .require_git(false)
        .sort_by_file_path(|a, b| a.cmp(b));

    for entry in builder.build() {
        let entry = entry.with_context(|| format!("walking {dir}"))?;
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            out.push(read_file(entry.path())?);
        }
    }
    Ok(())
}

/// Expand a shell-style glob (for quoted/unexpanded patterns). Matched files are
/// read; matched directories are walked. A pattern that matches nothing errors.
fn expand_glob(pattern: &str, walk: WalkOpts, out: &mut Vec<Input>) -> anyhow::Result<()> {
    // Mirror the walker: a leading-dot file matches `*` only with --hidden.
    let options = glob::MatchOptions {
        require_literal_leading_dot: !walk.hidden,
        ..glob::MatchOptions::new()
    };
    let paths = glob::glob_with(pattern, options)
        .map_err(|e| anyhow!("invalid glob pattern {pattern:?}: {e}"))?;
    let before = out.len();
    let mut matches: Vec<std::path::PathBuf> = Vec::new();
    for entry in paths {
        matches.push(entry.map_err(|e| anyhow!("glob error for {pattern:?}: {e}"))?);
    }
    matches.sort();
    for path in matches {
        if path.is_dir() {
            walk_dir(&path.to_string_lossy(), walk, out)?;
        } else if path.is_file() {
            out.push(read_file(&path)?);
        }
    }
    if out.len() == before {
        bail!("no files matched glob: {pattern}");
    }
    Ok(())
}

fn read_file(path: &Path) -> anyhow::Result<Input> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Input {
        label: path.to_string_lossy().into_owned(),
        bytes,
    })
}

/// Whether a positional looks like a glob pattern rather than a literal path.
fn has_glob_meta(s: &str) -> bool {
    s.contains(['*', '?', '['])
}
