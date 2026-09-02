use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use codex_route::config::{ConfigError, ScanConfig};
use codex_route::index::{IndexError, ResolveError, SessionWorkspaceIndex};

#[derive(Debug, Parser)]
#[command(
    name = "codex-route",
    version,
    about = "Resolve Codex session IDs to recorded workspaces"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Resolve one Codex session ID from local rollout metadata.
    Resolve(ResolveArgs),
}

#[derive(Debug, Args)]
struct ResolveArgs {
    /// Codex session tree identifier.
    #[arg(long)]
    session_id: String,
    /// Override the Codex home directory.
    #[arg(long)]
    codex_home: Option<PathBuf>,
    /// Maximum decompressed rollout prefix to inspect.
    #[arg(long)]
    max_rollout_bytes: Option<u64>,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Resolve(args) => {
            let config = ScanConfig::from_cli(args.codex_home, args.max_rollout_bytes)?;
            let index = SessionWorkspaceIndex::build(&config)?;
            let lookup = index.resolve(&args.session_id)?;
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            serde_json::to_writer_pretty(&mut handle, &lookup).map_err(CliError::Json)?;
            handle.write_all(b"\n").map_err(CliError::Output)?;
            Ok(())
        }
    }
}

#[derive(Debug)]
enum CliError {
    Config(ConfigError),
    Index(IndexError),
    Resolve(ResolveError),
    Json(serde_json::Error),
    Output(io::Error),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Resolve(ResolveError::SessionNotFound(_)) => 3,
            Self::Config(_) | Self::Resolve(ResolveError::EmptySessionId) => 2,
            Self::Index(_) | Self::Json(_) | Self::Output(_) => 4,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(error) => error.fmt(formatter),
            Self::Index(error) => error.fmt(formatter),
            Self::Resolve(error) => error.fmt(formatter),
            Self::Json(error) => write!(formatter, "failed to serialize output: {error}"),
            Self::Output(error) => write!(formatter, "failed to write output: {error}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<ConfigError> for CliError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<IndexError> for CliError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<ResolveError> for CliError {
    fn from(error: ResolveError) -> Self {
        Self::Resolve(error)
    }
}
