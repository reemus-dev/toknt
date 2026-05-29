//! The counting path: map CLI flags onto [`toknt::CountOptions`], resolve the
//! model column(s) (flags > `TOKNT_MODEL`, else `--approx`), gather inputs, and
//! count every (input × model) cell into a [`Report`] for [`super::render`].

use anyhow::bail;
use toknt::{
    count, count_approx, ApproxPolicy, ApproxReason, CountOptions, CountResult, NetworkPolicy,
    TokntError,
};

use super::input::{self, WalkOpts};
use super::render::{self, Mode};
use super::{CountArgs, APPROX_PROXY};

/// Model-independent statistics for one input (`--stats`).
pub struct Stats {
    pub chars: usize,
    pub words: usize,
    pub bytes: usize,
}

/// The outcome of one (input × model) cell.
pub enum Cell {
    Ok(Box<CountResult>),
    Err(String),
}

/// One input across every model column.
pub struct Row {
    pub label: String,
    pub cells: Vec<Cell>,
    /// `None` when the input was not valid UTF-8 (so every cell is an error).
    pub stats: Option<Stats>,
}

/// The full count matrix plus what `render` needs to format it.
pub struct Report {
    /// Column labels: the requested model ids, or a single `(approx)` column.
    pub models: Vec<String>,
    pub rows: Vec<Row>,
    pub stats_on: bool,
}

/// How a column is counted.
enum Column {
    /// A resolvable model id, counted via [`toknt::count`].
    Model(String),
    /// The no-model `--approx` column, counted via [`toknt::count_approx`].
    ApproxNoModel,
}

/// Run the counting path. Returns the intended exit code (0, or 1 when at least
/// one cell failed but other output was still produced). Pre-count failures
/// (no model, no input, missing file) surface as `Err` for `main` to print.
pub fn run(args: CountArgs) -> anyhow::Result<u8> {
    // 1. Resolve the model column(s): repeated -m wins over TOKNT_MODEL.
    let models: Vec<String> = if !args.model.is_empty() {
        args.model.clone()
    } else if let Some(env) = env_model() {
        vec![env]
    } else {
        Vec::new()
    };

    let no_model = models.is_empty();
    if no_model && !args.approx {
        bail!(
            "no model specified — pass -m <model>, set TOKNT_MODEL, or use --approx for an \
             estimate. See: toknt models"
        );
    }

    // 2. Map flags onto the library's policy surface (it reads keys from env).
    let mut opts = CountOptions::default();
    if args.offline {
        opts.network = NetworkPolicy::Offline;
    }
    if args.approx {
        opts.approx = ApproxPolicy::AllowApprox {
            proxy_encoding: APPROX_PROXY,
            // The library overrides this with the precise reason derived from
            // the actual failure; it is only a fallback that never gets used.
            reason: ApproxReason::UnsupportedModel,
        };
    }

    // 3. Gather inputs (errors here are fatal CLI-boundary errors).
    let inputs = input::gather(
        &args.inputs,
        args.text.as_deref(),
        WalkOpts {
            no_ignore: args.no_ignore,
            hidden: args.hidden,
        },
    )?;

    // 4. Build the columns and count every cell.
    let columns: Vec<Column> = if no_model {
        vec![Column::ApproxNoModel]
    } else {
        models.iter().cloned().map(Column::Model).collect()
    };
    let column_labels: Vec<String> = if no_model {
        vec!["(approx)".to_string()]
    } else {
        models.clone()
    };

    let rows = inputs
        .into_iter()
        .map(|item| count_row(item, &columns, &opts))
        .collect();

    let report = Report {
        models: column_labels,
        rows,
        stats_on: args.stats,
    };

    // 5. Render in the requested mode; the exit code reflects any cell failure.
    let mode = if args.json {
        Mode::Json
    } else if args.verbose {
        Mode::Verbose
    } else {
        Mode::Default
    };
    Ok(render::report(&report, mode))
}

fn count_row(item: input::Input, columns: &[Column], opts: &CountOptions) -> Row {
    let text = match std::str::from_utf8(&item.bytes) {
        Ok(text) => text,
        Err(e) => {
            // One canonical invalid-UTF-8 error, reused for every column.
            let msg = TokntError::InvalidUtf8 {
                what: item.label.clone(),
                offset: e.valid_up_to(),
            }
            .to_string();
            let cells = columns.iter().map(|_| Cell::Err(msg.clone())).collect();
            return Row {
                label: item.label,
                cells,
                stats: None,
            };
        }
    };

    let stats = Some(Stats {
        chars: text.chars().count(),
        words: text.split_whitespace().count(),
        bytes: item.bytes.len(),
    });

    let cells = columns
        .iter()
        .map(|col| match col {
            Column::Model(model) => match count(text, model, opts) {
                Ok(result) => Cell::Ok(Box::new(result)),
                Err(err) => Cell::Err(err.to_string()),
            },
            Column::ApproxNoModel => Cell::Ok(Box::new(count_approx(
                text,
                "(approx)",
                APPROX_PROXY,
                ApproxReason::NoModel,
            ))),
        })
        .collect();

    Row {
        label: item.label,
        cells,
        stats,
    }
}

/// `TOKNT_MODEL`, when set and non-empty.
fn env_model() -> Option<String> {
    std::env::var("TOKNT_MODEL").ok().filter(|s| !s.is_empty())
}
