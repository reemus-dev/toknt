//! The CLI command tree (clap) and top-level dispatch. Submodules carry the
//! work: [`input`] routes file/glob/dir/`-t`/stdin sources, [`count`] drives the
//! library over the (input × model) matrix, [`render`] formats every output
//! mode, and [`commands`] backs the `models`/`pull`/`cache`/`completions`
//! subcommands.

use clap::{Args, Parser, Subcommand, ValueEnum};

mod commands;
mod count;
mod input;
mod render;

/// The default tiktoken proxy used for `--approx` estimates. o200k_base is the
/// modern OpenAI encoding; it stands in when no exact/provider count is had.
const APPROX_PROXY: toknt::ProxyEncoding = toknt::ProxyEncoding::O200kBase;

#[derive(Parser, Debug)]
#[command(
    name = "toknt",
    version,
    about = "Accurate, model-specific token counts for files, stdin, or inline text.",
    long_about = "toknt prints an accurate token count for UTF-8 text — file(s), globs, \
directories, inline -t, or piped stdin — against one or more specific models.\n\n\
Offline tokenizers (tiktoken, Hugging Face) are exact; Anthropic/Gemini report a \
provider count/estimate over content + a small message envelope. Approximations happen \
only with --approx and are always clearly labeled.\n\n\
A model is required (via -m or TOKNT_MODEL) unless --approx is used.",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    after_help = "EXAMPLES:\n  \
toknt -m o200k_base file.txt              exact, offline\n  \
echo \"hello world\" | toknt -m gpt-5.2    stdin, OpenAI alias\n  \
toknt -m claude-opus-4-8 file.txt         Anthropic provider estimate (needs key)\n  \
toknt -m gpt-5.2 -m claude-opus-4-8 f.txt comparison table\n  \
toknt -m o200k_base src/ --stats          recurse a dir, add char/word stats\n  \
toknt --approx file.txt                   labeled estimate, no model needed"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub count: CountArgs,
}

/// Options for the default (no-subcommand) counting path.
#[derive(Args, Debug)]
pub struct CountArgs {
    /// File paths, globs, or directories. Omit when piping stdin.
    #[arg(value_name = "INPUTS")]
    pub inputs: Vec<String>,

    /// Model/encoding to count for. Repeat for a comparison table.
    #[arg(short, long, value_name = "ID")]
    pub model: Vec<String>,

    /// Count this inline string instead of files/stdin.
    #[arg(short, long, value_name = "STR", conflicts_with = "inputs")]
    pub text: Option<String>,

    /// Accept a clearly-labeled approximation (covers no-model and no-API-key).
    #[arg(long)]
    pub approx: bool,

    /// Never touch the network: API models error, uncached HF errors, tiktoken still works.
    #[arg(long)]
    pub offline: bool,

    /// Machine-readable JSON (model, strategy, basis, accuracy, approximation, tokens, stats).
    #[arg(long, conflicts_with = "verbose")]
    pub json: bool,

    /// Human-readable breakdown (model, strategy, basis, accuracy).
    #[arg(short, long)]
    pub verbose: bool,

    /// Include chars/words/bytes/tokens-per-word.
    #[arg(long)]
    pub stats: bool,

    /// Do not honor .gitignore when walking directories.
    #[arg(long)]
    pub no_ignore: bool,

    /// Include hidden files when walking directories.
    #[arg(long)]
    pub hidden: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List or search the known-model registry.
    Models {
        /// Optional case-insensitive filter (matches the pattern or its detail).
        #[arg(value_name = "QUERY")]
        query: Option<String>,
    },

    /// Pre-fetch & cache an open-weight (Hugging Face) tokenizer for offline use.
    Pull {
        /// A Hugging Face model id (e.g. org/name) or an explicit hf:org/name.
        #[arg(value_name = "MODEL")]
        model: String,
    },

    /// Manage the tokenizer cache.
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Print shell completions for bash, zsh, fish, or powershell.
    Completions {
        /// The shell to generate a completion script for.
        #[arg(value_name = "SHELL")]
        shell: ShellArg,
    },
}

#[derive(Subcommand, Debug)]
pub enum CacheAction {
    /// List cached open-weight tokenizers.
    Ls,
    /// Remove a cached tokenizer.
    Rm {
        /// The model/repo to remove (e.g. org/name).
        #[arg(value_name = "MODEL")]
        model: String,
    },
    /// Print the resolved cache directory path.
    Path,
}

/// The shells `completions` can target. Mirrors the subset of
/// [`clap_complete::Shell`] the spec requires.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ShellArg {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

impl From<ShellArg> for clap_complete::Shell {
    fn from(s: ShellArg) -> Self {
        match s {
            ShellArg::Bash => clap_complete::Shell::Bash,
            ShellArg::Zsh => clap_complete::Shell::Zsh,
            ShellArg::Fish => clap_complete::Shell::Fish,
            ShellArg::Powershell => clap_complete::Shell::PowerShell,
        }
    }
}

/// Route a parsed [`Cli`] to its handler. Returns the intended process exit code
/// (0 success, 1 when a count failed but output was still produced); fatal
/// CLI-boundary errors are returned as `Err` and printed by `main`.
pub fn dispatch(cli: Cli) -> anyhow::Result<u8> {
    match cli.command {
        Some(Command::Models { query }) => commands::models(query.as_deref()),
        Some(Command::Pull { model }) => commands::pull(&model),
        Some(Command::Cache { action }) => commands::cache(action),
        Some(Command::Completions { shell }) => {
            commands::completions(shell);
            Ok(0)
        }
        None => count::run(cli.count),
    }
}
