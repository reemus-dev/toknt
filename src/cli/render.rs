//! Output formatting for the count matrix. Honesty rules live here: an
//! approximation is always marked (`~N`, never a bare integer), and a
//! comparison never implies two different bases are a like-for-like delta.
//!
//! Stream discipline: successful counts render to stdout; in the human modes
//! per-cell errors render to stderr (so a partial table stays parseable); in
//! JSON the whole document — errors included — is one object on stdout. The
//! returned `u8` is the process exit code (1 if any cell failed).

use serde::Serialize;
use serde_json::{json, Value};
use toknt::{Accuracy, Basis, CountResult};

use super::count::{Cell, Report, Stats};

pub enum Mode {
    Default,
    Verbose,
    Json,
}

pub fn report(rep: &Report, mode: Mode) -> u8 {
    match mode {
        Mode::Json => render_json(rep),
        Mode::Verbose => render_verbose(rep),
        Mode::Default => render_default(rep),
    }
}

// ---- default (terminal) mode ---------------------------------------------

fn render_default(rep: &Report) -> u8 {
    let inputs = rep.rows.len();
    let models = rep.models.len();
    match (inputs, models) {
        (1, 1) => scalar(rep),
        (_, 1) => per_file(rep),
        (1, _) => comparison(rep),
        _ => matrix(rep),
    }
}

/// One input, one model: a bare integer (script-friendly), or a clearly-marked
/// approximation. `--stats` appends a compact metrics line.
fn scalar(rep: &Report) -> u8 {
    let row = &rep.rows[0];
    match &row.cells[0] {
        Cell::Ok(res) => {
            if res.accuracy == Accuracy::Approximate {
                println!("~{} (APPROXIMATE: {} proxy)", res.tokens, proxy_label(res));
            } else {
                println!("{}", res.tokens);
            }
            if rep.stats_on {
                if let Some(stats) = &row.stats {
                    println!(
                        "chars {}  words {}  bytes {}  tokens/word {:.2}",
                        stats.chars,
                        stats.words,
                        stats.bytes,
                        tokens_per_word(res.tokens, stats.words),
                    );
                }
            }
            0
        }
        Cell::Err(msg) => {
            eprintln!("toknt: {msg}");
            1
        }
    }
}

/// Many inputs, one model: a `wc`-like per-file breakdown plus a TOTAL. With
/// `--stats`, widen to chars/words/bytes/tokens-per-word columns.
fn per_file(rep: &Report) -> u8 {
    let mut code = 0u8;
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut total_tokens = 0usize;
    let mut total_chars = 0usize;
    let mut total_words = 0usize;
    let mut total_bytes = 0usize;
    let mut total_approx = false;
    let mut partial = false;

    let mut headers = vec!["TOKENS"];
    if rep.stats_on {
        headers.extend(["CHARS", "WORDS", "BYTES", "TOK/WORD"]);
    }
    headers.push("FILE");
    let mut trows: Vec<Vec<String>> = Vec::new();

    for row in &rep.rows {
        match &row.cells[0] {
            Cell::Ok(res) => {
                total_tokens += res.tokens;
                total_approx |= res.accuracy == Accuracy::Approximate;
                let mut tr = vec![token_cell(res)];
                if rep.stats_on {
                    let s = row.stats.as_ref();
                    let (c, w, b) = s.map(|s| (s.chars, s.words, s.bytes)).unwrap_or((0, 0, 0));
                    total_chars += c;
                    total_words += w;
                    total_bytes += b;
                    tr.push(c.to_string());
                    tr.push(w.to_string());
                    tr.push(b.to_string());
                    tr.push(format!("{:.2}", tokens_per_word(res.tokens, w)));
                }
                tr.push(row.label.clone());
                trows.push(tr);
            }
            Cell::Err(msg) => {
                code = 1;
                partial = true;
                let mut tr = vec!["—".to_string()];
                if rep.stats_on {
                    tr.extend(["—", "—", "—", "—"].iter().map(|s| s.to_string()));
                }
                tr.push(row.label.clone());
                trows.push(tr);
                errors.push((row.label.clone(), msg.clone()));
            }
        }
    }

    // TOTAL row.
    let mut total_row = vec![mark(total_tokens, total_approx)];
    if rep.stats_on {
        total_row.push(total_chars.to_string());
        total_row.push(total_words.to_string());
        total_row.push(total_bytes.to_string());
        total_row.push(format!("{:.2}", tokens_per_word(total_tokens, total_words)));
    }
    total_row.push(if partial {
        "TOTAL (partial)".into()
    } else {
        "TOTAL".into()
    });
    trows.push(total_row);

    let mut aligns = vec![Align::Right];
    if rep.stats_on {
        aligns.extend([Align::Right, Align::Right, Align::Right, Align::Right]);
    }
    aligns.push(Align::Left);
    print_table(&headers, &trows, &aligns);
    emit_errors(&errors);
    code
}

/// One input, many models: an aligned comparison table. Each row labels its
/// basis; a footnote flags that envelope-based rows are not a like-for-like
/// delta against raw-content rows.
fn comparison(rep: &Report) -> u8 {
    let row = &rep.rows[0];
    let mut code = 0u8;
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut any_envelope = false;

    let mut headers = vec!["MODEL", "TOKENS", "BASIS", "ACCURACY"];
    if rep.stats_on {
        headers.push("TOK/WORD");
    }
    let mut trows: Vec<Vec<String>> = Vec::new();

    for (i, cell) in row.cells.iter().enumerate() {
        let model = rep.models[i].clone();
        match cell {
            Cell::Ok(res) => {
                any_envelope |= res.basis == Basis::ContentEnvelope;
                let mut tr = vec![
                    model,
                    token_cell(res),
                    label(&res.basis),
                    label(&res.accuracy),
                ];
                if rep.stats_on {
                    let w = row.stats.as_ref().map(|s| s.words).unwrap_or(0);
                    tr.push(format!("{:.2}", tokens_per_word(res.tokens, w)));
                }
                trows.push(tr);
            }
            Cell::Err(msg) => {
                code = 1;
                let mut tr = vec![model.clone(), "ERROR".into(), "—".into(), "—".into()];
                if rep.stats_on {
                    tr.push("—".into());
                }
                trows.push(tr);
                errors.push((model, msg.clone()));
            }
        }
    }

    let mut aligns = vec![Align::Left, Align::Right, Align::Left, Align::Left];
    if rep.stats_on {
        aligns.push(Align::Right);
    }
    print_table(&headers, &trows, &aligns);
    if any_envelope {
        println!(
            "\nnote: content+envelope rows include the provider's message envelope — not a \
             like-for-like delta against raw-content rows."
        );
    }
    if rep.stats_on {
        if let Some(s) = &row.stats {
            println!(
                "input: chars {}  words {}  bytes {}",
                s.chars, s.words, s.bytes
            );
        }
    }
    emit_errors(&errors);
    code
}

/// Many inputs, many models: a token matrix (files × models) with a TOTAL row,
/// a per-model basis legend, and an optional per-file stats table.
fn matrix(rep: &Report) -> u8 {
    let mut code = 0u8;
    let mut errors: Vec<(String, String)> = Vec::new();
    let cols = rep.models.len();
    let mut totals = vec![0usize; cols];
    let mut col_approx = vec![false; cols];
    let mut col_partial = vec![false; cols];
    // A representative basis/accuracy per column, from its first successful cell.
    let mut col_basis: Vec<Option<String>> = vec![None; cols];

    let headers: Vec<&str> = std::iter::once("FILE")
        .chain(rep.models.iter().map(String::as_str))
        .collect();
    let mut trows: Vec<Vec<String>> = Vec::new();

    for row in &rep.rows {
        let mut tr = vec![row.label.clone()];
        for (ci, cell) in row.cells.iter().enumerate() {
            match cell {
                Cell::Ok(res) => {
                    totals[ci] += res.tokens;
                    col_approx[ci] |= res.accuracy == Accuracy::Approximate;
                    if col_basis[ci].is_none() {
                        col_basis[ci] =
                            Some(format!("{} ({})", label(&res.basis), label(&res.accuracy)));
                    }
                    tr.push(token_cell(res));
                }
                Cell::Err(msg) => {
                    code = 1;
                    col_partial[ci] = true;
                    tr.push("—".into());
                    errors.push((format!("{} [{}]", row.label, rep.models[ci]), msg.clone()));
                }
            }
        }
        trows.push(tr);
    }

    // TOTAL row.
    let mut total_row = vec!["TOTAL".to_string()];
    for ci in 0..cols {
        let mut cell = mark(totals[ci], col_approx[ci]);
        if col_partial[ci] {
            cell.push('*');
        }
        total_row.push(cell);
    }
    trows.push(total_row);

    let mut aligns = vec![Align::Left];
    aligns.extend(std::iter::repeat_n(Align::Right, cols));
    print_table(&headers, &trows, &aligns);

    // Per-model basis legend; flag that mixed bases are not comparable.
    println!("\nbasis:");
    let mut any_envelope = false;
    for (ci, model) in rep.models.iter().enumerate() {
        let basis = col_basis[ci].clone().unwrap_or_else(|| "—".into());
        any_envelope |= basis.starts_with("content+envelope");
        println!("  {model}: {basis}");
    }
    if any_envelope {
        println!(
            "content+envelope rows include the provider's message envelope; bases differ — not \
             directly comparable."
        );
    }
    if col_partial.iter().any(|&p| p) {
        println!("* column total excludes failed input(s).");
    }

    if rep.stats_on {
        print_stats_table(rep);
    }
    emit_errors(&errors);
    code
}

/// A standalone per-file stats table (model-independent) for matrix `--stats`.
fn print_stats_table(rep: &Report) {
    let headers = ["FILE", "CHARS", "WORDS", "BYTES"];
    let mut trows: Vec<Vec<String>> = Vec::new();
    let (mut tc, mut tw, mut tb) = (0usize, 0usize, 0usize);
    for row in &rep.rows {
        if let Some(s) = &row.stats {
            tc += s.chars;
            tw += s.words;
            tb += s.bytes;
            trows.push(vec![
                row.label.clone(),
                s.chars.to_string(),
                s.words.to_string(),
                s.bytes.to_string(),
            ]);
        } else {
            trows.push(vec![row.label.clone(), "—".into(), "—".into(), "—".into()]);
        }
    }
    trows.push(vec![
        "TOTAL".into(),
        tc.to_string(),
        tw.to_string(),
        tb.to_string(),
    ]);
    println!("\nstats:");
    print_table(
        &headers,
        &trows,
        &[Align::Left, Align::Right, Align::Right, Align::Right],
    );
}

// ---- verbose mode ---------------------------------------------------------

fn render_verbose(rep: &Report) -> u8 {
    let mut code = 0u8;
    let mut first = true;
    for row in &rep.rows {
        for (ci, cell) in row.cells.iter().enumerate() {
            match cell {
                Cell::Ok(res) => {
                    if !first {
                        println!();
                    }
                    first = false;
                    println!("{}", row.label);
                    kv("model", &res.model);
                    kv("resolved", &res.resolved_model);
                    kv("strategy", &label(&res.strategy));
                    if let Some(enc) = &res.encoding {
                        kv("encoding", enc);
                    }
                    if let Some(rev) = &res.revision {
                        kv("revision", rev);
                    }
                    if let Some(ast) = res.add_special_tokens {
                        kv("special-tokens", if ast { "included" } else { "omitted" });
                    }
                    kv("basis", &label(&res.basis));
                    kv("accuracy", &label(&res.accuracy));
                    if let Some(a) = res.approximation {
                        kv(
                            "approximation",
                            &format!(
                                "{} proxy · reason: {}",
                                label(&a.proxy_encoding),
                                label(&a.reason)
                            ),
                        );
                    }
                    kv("tokens", &res.tokens.to_string());
                    if rep.stats_on {
                        if let Some(s) = &row.stats {
                            kv("chars", &s.chars.to_string());
                            kv("words", &s.words.to_string());
                            kv("bytes", &s.bytes.to_string());
                            kv(
                                "tokens/word",
                                &format!("{:.2}", tokens_per_word(res.tokens, s.words)),
                            );
                        }
                    }
                }
                Cell::Err(msg) => {
                    code = 1;
                    eprintln!("{} [{}]: {}", row.label, rep.models[ci], msg);
                }
            }
        }
    }
    code
}

fn kv(key: &str, val: &str) {
    println!("  {:<16}{}", format!("{key}:"), val);
}

// ---- json mode ------------------------------------------------------------

fn render_json(rep: &Report) -> u8 {
    let mut code = 0u8;
    let mut results: Vec<Value> = Vec::new();
    let cols = rep.models.len();
    let mut totals = vec![0usize; cols];
    let mut complete = vec![true; cols];
    let multi_input = rep.rows.len() > 1;

    for row in &rep.rows {
        for (ci, cell) in row.cells.iter().enumerate() {
            match cell {
                Cell::Ok(res) => {
                    totals[ci] += res.tokens;
                    let mut obj = serde_json::to_value(res.as_ref()).unwrap_or(Value::Null);
                    if let Value::Object(map) = &mut obj {
                        map.insert("input".into(), json!(row.label));
                        if rep.stats_on {
                            if let Some(s) = &row.stats {
                                map.insert("stats".into(), stats_json(s, res.tokens));
                            }
                        }
                    }
                    results.push(obj);
                }
                Cell::Err(msg) => {
                    code = 1;
                    complete[ci] = false;
                    results.push(json!({
                        "input": row.label,
                        "model": rep.models[ci],
                        "error": msg,
                    }));
                }
            }
        }
    }

    let total = if multi_input {
        Value::Array(
            (0..cols)
                .map(|ci| {
                    json!({
                        "model": rep.models[ci],
                        "tokens": totals[ci],
                        "complete": complete[ci],
                    })
                })
                .collect(),
        )
    } else {
        Value::Null
    };

    let doc = json!({ "results": results, "total": total });
    println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
    code
}

fn stats_json(stats: &Stats, tokens: usize) -> Value {
    json!({
        "chars": stats.chars,
        "words": stats.words,
        "bytes": stats.bytes,
        "tokens_per_word": round2(tokens_per_word(tokens, stats.words)),
    })
}

// ---- shared helpers -------------------------------------------------------

/// A count cell for tables: approximations are marked with a leading `~`.
fn token_cell(res: &CountResult) -> String {
    mark(res.tokens, res.accuracy == Accuracy::Approximate)
}

fn mark(tokens: usize, approx: bool) -> String {
    if approx {
        format!("~{tokens}")
    } else {
        tokens.to_string()
    }
}

/// The proxy encoding name backing an approximate count (for labels).
fn proxy_label(res: &CountResult) -> String {
    res.encoding.clone().unwrap_or_else(|| "proxy".into())
}

/// Render a serde-serializable enum (Strategy/Basis/Accuracy/…) as its string.
fn label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "?".into())
}

fn tokens_per_word(tokens: usize, words: usize) -> f64 {
    if words == 0 {
        0.0
    } else {
        tokens as f64 / words as f64
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn emit_errors(errors: &[(String, String)]) {
    for (label, msg) in errors {
        eprintln!("{label}: {msg}");
    }
}

// ---- minimal table printer ------------------------------------------------

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

/// Print a header + rows with per-column width and alignment. Column widths are
/// measured in characters (so non-ASCII labels still align); columns are joined
/// by two spaces.
fn print_table(headers: &[&str], rows: &[Vec<String>], aligns: &[Align]) {
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(cols) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    print_row(&header_cells, &widths, aligns);
    for row in rows {
        print_row(row, &widths, aligns);
    }
}

fn print_row(cells: &[String], widths: &[usize], aligns: &[Align]) {
    let mut out = String::new();
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        let pad = widths[i].saturating_sub(cell.chars().count());
        match aligns.get(i).copied().unwrap_or(Align::Left) {
            Align::Left => {
                out.push_str(cell);
                // No trailing pad on the last column.
                if i + 1 < cells.len() {
                    out.extend(std::iter::repeat_n(' ', pad));
                }
            }
            Align::Right => {
                out.extend(std::iter::repeat_n(' ', pad));
                out.push_str(cell);
            }
        }
    }
    println!("{out}");
}
