use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use codex_route::cc_switch_import::{CcSwitchImportError, CcSwitchImporter, ConflictPolicy};
use codex_route::config::{ConfigError, ScanConfig};
use codex_route::index::{IndexError, ResolveError, SessionWorkspaceIndex};
use codex_route::lifecycle::{self, LifecycleError, LifecyclePaths};
use codex_route::provider::ProviderSummary;
use codex_route::provider_store::UpsertRouteRuleOutcome;
use codex_route::provider_store::{ProviderStore, ProviderStoreError};
use codex_route::route::{self, RouteStartupError};

#[derive(Debug, Parser)]
#[command(
    name = "codex-route",
    version,
    about = "Inspect Codex sessions and recorded workspaces"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Resolve one Codex session ID from local rollout metadata.
    Resolve(ResolveArgs),
    /// List all unique Codex session IDs from local rollout metadata.
    List(ListArgs),
    /// Manage locally stored Codex upstream providers.
    Provider(ProviderArgs),
    /// Run the local loopback Responses route.
    Route(RouteArgs),
}

#[derive(Debug, Args)]
struct ProviderArgs {
    #[command(subcommand)]
    command: ProviderCommand,
}

#[derive(Debug, Subcommand)]
enum ProviderCommand {
    /// List stored providers without configuration payloads.
    List(ProviderListArgs),
    /// Show one stored provider.
    Show(ProviderShowArgs),
    /// Import Codex providers from a cc-switch SQLite database.
    #[command(name = "import-cc-switch")]
    ImportCcSwitch(ProviderImportArgs),
}

#[derive(Debug, Args)]
struct RouteArgs {
    #[command(subcommand)]
    command: RouteCommand,
}

#[derive(Debug, Subcommand)]
enum RouteCommand {
    /// Serve native Codex Responses requests through a stored provider.
    Serve(RouteServeArgs),
    /// Start the route in the background and connect Codex to it.
    Activate(RouteActivateArgs),
    /// Show route process and Codex configuration status.
    Status(RouteStatusArgs),
    /// Stop the background route and restore Codex configuration.
    Deactivate(RouteDeactivateArgs),
    /// Manage workspace-to-provider routing rules.
    Rule(RouteRuleArgs),
}

#[derive(Debug, Args)]
struct RouteServeArgs {
    #[command(flatten)]
    scan: ScanArgs,
    /// Directory containing codex-route.db.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Provider identifier to use for every request.
    #[arg(long)]
    provider: Option<String>,
    /// Loopback TCP port.
    #[arg(long, default_value_t = route::DEFAULT_ROUTE_PORT)]
    port: u16,
    /// Internal lifecycle lock path used by `route activate`.
    #[arg(long, hide = true)]
    lifecycle_lock: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RouteActivateArgs {
    #[command(flatten)]
    scan: ScanArgs,
    /// Directory containing codex-route.db and lifecycle state.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Provider identifier to use for every request.
    #[arg(long)]
    provider: Option<String>,
    /// Loopback TCP port.
    #[arg(long, default_value_t = route::DEFAULT_ROUTE_PORT)]
    port: u16,
}

#[derive(Debug, Args)]
struct RouteStatusArgs {
    /// Directory containing codex-route.db and lifecycle state.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Override the Codex home used when no active state exists.
    #[arg(long)]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RouteDeactivateArgs {
    /// Directory containing codex-route.db and lifecycle state.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Override the Codex home used when no active state exists.
    #[arg(long)]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RouteRuleArgs {
    #[command(subcommand)]
    command: RouteRuleCommand,
}

#[derive(Debug, Subcommand)]
enum RouteRuleCommand {
    /// Add or update a workspace-to-provider route rule.
    Add(RouteRuleAddArgs),
    /// List workspace-to-provider route rules.
    List(RouteRuleListArgs),
    /// Remove a workspace-to-provider route rule.
    Remove(RouteRuleRemoveArgs),
}

#[derive(Debug, Args)]
struct RouteRuleAddArgs {
    /// Workspace path to route.
    #[arg(long)]
    workspace: PathBuf,
    /// Stored provider identifier.
    #[arg(long)]
    provider: String,
    /// Replace an existing rule for this workspace.
    #[arg(long)]
    replace: bool,
    /// Directory containing codex-route.db.
    #[arg(long)]
    data_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RouteRuleListArgs {
    /// Directory containing codex-route.db.
    #[arg(long)]
    data_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RouteRuleRemoveArgs {
    /// Workspace path to remove.
    #[arg(long)]
    workspace: PathBuf,
    /// Directory containing codex-route.db.
    #[arg(long)]
    data_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ProviderListArgs {
    /// Directory containing codex-route.db.
    #[arg(long)]
    data_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ProviderShowArgs {
    /// Local provider identifier.
    id: String,
    /// Directory containing codex-route.db.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Include credential fields in the output.
    #[arg(long)]
    reveal_secrets: bool,
}

#[derive(Debug, Args)]
struct ProviderImportArgs {
    /// Directory containing codex-route.db.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Path to the cc-switch database.
    #[arg(long)]
    cc_switch_db: Option<PathBuf>,
    /// How to handle an existing provider.
    #[arg(long, value_enum, default_value_t = ConflictPolicy::Skip)]
    on_conflict: ConflictPolicy,
}

#[derive(Debug, Args)]
struct ScanArgs {
    /// Override the Codex home directory.
    #[arg(long)]
    codex_home: Option<PathBuf>,
    /// Maximum decompressed rollout prefix to inspect.
    #[arg(long)]
    max_rollout_bytes: Option<u64>,
}

#[derive(Debug, Args)]
struct ResolveArgs {
    #[command(flatten)]
    scan: ScanArgs,
    /// Codex session tree identifier.
    #[arg(long)]
    session_id: String,
}

#[derive(Debug, Args)]
struct ListArgs {
    #[command(flatten)]
    scan: ScanArgs,
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
            let config = ScanConfig::from_cli(args.scan.codex_home, args.scan.max_rollout_bytes)?;
            let index = SessionWorkspaceIndex::build(&config)?;
            let lookup = index.resolve(&args.session_id)?;
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            serde_json::to_writer_pretty(&mut handle, &lookup).map_err(CliError::Json)?;
            handle.write_all(b"\n").map_err(CliError::Output)?;
            Ok(())
        }
        Command::List(args) => {
            let config = ScanConfig::from_cli(args.scan.codex_home, args.scan.max_rollout_bytes)?;
            let index = SessionWorkspaceIndex::build(&config)?;
            let session_ids = index.session_ids();
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            serde_json::to_writer_pretty(&mut handle, &session_ids).map_err(CliError::Json)?;
            handle.write_all(b"\n").map_err(CliError::Output)?;
            Ok(())
        }
        Command::Provider(args) => run_provider_command(args),
        Command::Route(args) => run_route_command(args),
    }
}

fn run_route_command(args: RouteArgs) -> Result<(), CliError> {
    match args.command {
        RouteCommand::Serve(args) => run_route_serve(args),
        RouteCommand::Activate(args) => run_route_activate(args),
        RouteCommand::Status(args) => run_route_status(args),
        RouteCommand::Deactivate(args) => run_route_deactivate(args),
        RouteCommand::Rule(args) => run_route_rule_command(args),
    }
}

fn run_route_serve(args: RouteServeArgs) -> Result<(), CliError> {
    let _lock = match args.lifecycle_lock.as_deref() {
        Some(path) => Some(lifecycle::DaemonLock::acquire(path)?),
        None => None,
    };
    let store = Arc::new(open_provider_store(args.data_dir)?);
    let scan = ScanConfig::from_cli(args.scan.codex_home, args.scan.max_rollout_bytes)?;
    let state = route::RouteState::with_scan_config(store, args.provider, scan)?;
    state.validate_selection()?;
    eprintln!("listening on 127.0.0.1:{}", args.port);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(CliError::Runtime)?;
    runtime
        .block_on(route::serve(state, args.port))
        .map_err(CliError::RouteServe)
}

fn run_route_activate(args: RouteActivateArgs) -> Result<(), CliError> {
    let data_dir = args.data_dir.unwrap_or_else(default_provider_data_dir);
    let scan = ScanConfig::from_cli(args.scan.codex_home, args.scan.max_rollout_bytes)?;
    let paths = LifecyclePaths::new(data_dir, scan.codex_home.clone());
    let result = lifecycle::activate(lifecycle::ActivateOptions {
        paths,
        provider_id: args.provider,
        port: args.port,
        scan_config: scan,
    })?;
    write_json(&result)
}

fn run_route_status(args: RouteStatusArgs) -> Result<(), CliError> {
    let data_dir = args.data_dir.unwrap_or_else(default_provider_data_dir);
    let codex_home = resolve_lifecycle_codex_home(&data_dir, args.codex_home)?;
    let result = lifecycle::status(lifecycle::StatusOptions {
        paths: LifecyclePaths::new(data_dir, codex_home),
    })?;
    write_json(&result)
}

fn run_route_deactivate(args: RouteDeactivateArgs) -> Result<(), CliError> {
    let data_dir = args.data_dir.unwrap_or_else(default_provider_data_dir);
    let codex_home = resolve_lifecycle_codex_home(&data_dir, args.codex_home)?;
    let result = lifecycle::deactivate(lifecycle::DeactivateOptions {
        paths: LifecyclePaths::new(data_dir, codex_home),
    })?;
    write_json(&result)
}

fn resolve_lifecycle_codex_home(
    data_dir: &std::path::Path,
    requested: Option<PathBuf>,
) -> Result<PathBuf, CliError> {
    if let Some(path) = requested {
        if !path.is_absolute() {
            return Err(CliError::Config(ConfigError::RelativeCodexHome(path)));
        }
        return Ok(path);
    }
    if let Some(path) = lifecycle::codex_home_from_state(data_dir) {
        return Ok(path);
    }
    Ok(ScanConfig::from_cli(None, None)?.codex_home)
}

fn run_route_rule_command(args: RouteRuleArgs) -> Result<(), CliError> {
    match args.command {
        RouteRuleCommand::Add(args) => {
            let store = open_provider_store(args.data_dir)?;
            let outcome = store.upsert_route_rule(&args.workspace, &args.provider, args.replace)?;
            let action = match outcome {
                UpsertRouteRuleOutcome::Inserted => "inserted",
                UpsertRouteRuleOutcome::Replaced => "replaced",
            };
            let rule = store
                .get_route_rule(&args.workspace)?
                .ok_or_else(|| CliError::RouteRuleNotFound(args.workspace.clone()))?;
            write_json(&serde_json::json!({
                "action": action,
                "rule": rule,
            }))
        }
        RouteRuleCommand::List(args) => {
            let store = open_provider_store(args.data_dir)?;
            write_json(&store.list_route_rules()?)
        }
        RouteRuleCommand::Remove(args) => {
            let store = open_provider_store(args.data_dir)?;
            let rule = match store.remove_route_rule(&args.workspace) {
                Ok(rule) => rule,
                Err(ProviderStoreError::RouteRuleNotFound(path)) => {
                    return Err(CliError::RouteRuleNotFound(path));
                }
                Err(error) => return Err(error.into()),
            };
            write_json(&rule)
        }
    }
}

fn run_provider_command(args: ProviderArgs) -> Result<(), CliError> {
    match args.command {
        ProviderCommand::List(args) => {
            let store = open_provider_store(args.data_dir)?;
            let summaries: Vec<ProviderSummary> =
                store.list()?.iter().map(ProviderSummary::from).collect();
            write_json(&summaries)
        }
        ProviderCommand::Show(args) => {
            let store = open_provider_store(args.data_dir)?;
            let provider = store
                .get(&args.id)?
                .ok_or_else(|| CliError::ProviderNotFound(args.id.clone()))?;
            let mut value = serde_json::to_value(provider).map_err(CliError::Json)?;
            if !args.reveal_secrets {
                redact_json_secrets(&mut value);
            }
            write_json(&value)
        }
        ProviderCommand::ImportCcSwitch(args) => {
            let store = open_provider_store(args.data_dir)?;
            let source = args
                .cc_switch_db
                .unwrap_or_else(CcSwitchImporter::discover_default_db);
            let scan = CcSwitchImporter::new(source).read_codex_providers()?;
            let report = store.import_scan_transaction(&scan, args.on_conflict)?;
            write_json(&report)
        }
    }
}

fn open_provider_store(data_dir: Option<PathBuf>) -> Result<ProviderStore, CliError> {
    let directory = data_dir.unwrap_or_else(default_provider_data_dir);
    Ok(ProviderStore::open(directory.join("codex-route.db"))?)
}

fn default_provider_data_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("codex-route")
}

fn write_json<T: serde::Serialize>(value: &T) -> Result<(), CliError> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, value).map_err(CliError::Json)?;
    handle.write_all(b"\n").map_err(CliError::Output)
}

fn redact_json_secrets(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if is_secret_key(key) {
                    *child = serde_json::Value::String("[REDACTED]".to_string());
                } else if key == "config" {
                    redact_toml_config(child);
                } else {
                    redact_json_secrets(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json_secrets(item);
            }
        }
        _ => {}
    }
}

fn is_secret_key(key: &str) -> bool {
    matches!(
        key,
        "OPENAI_API_KEY"
            | "api_key"
            | "apiKey"
            | "experimental_bearer_token"
            | "access_token"
            | "refresh_token"
    )
}

fn redact_toml_config(value: &mut serde_json::Value) {
    let Some(config) = value.as_str() else {
        redact_json_secrets(value);
        return;
    };
    let Ok(mut document) = config.parse::<toml::Value>() else {
        return;
    };
    redact_toml_value(&mut document);
    *value = serde_json::Value::String(document.to_string());
}

fn redact_toml_value(value: &mut toml::Value) {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table.iter_mut() {
                if is_secret_key(key) {
                    *child = toml::Value::String("[REDACTED]".to_string());
                } else {
                    redact_toml_value(child);
                }
            }
        }
        toml::Value::Array(items) => {
            for item in items {
                redact_toml_value(item);
            }
        }
        _ => {}
    }
}

#[derive(Debug)]
enum CliError {
    Config(ConfigError),
    Index(IndexError),
    Resolve(ResolveError),
    Json(serde_json::Error),
    Output(io::Error),
    ProviderStore(ProviderStoreError),
    CcSwitchImport(CcSwitchImportError),
    Lifecycle(LifecycleError),
    ProviderNotFound(String),
    RouteRuleNotFound(PathBuf),
    RouteStartup(RouteStartupError),
    RouteServe(io::Error),
    Runtime(io::Error),
}

impl CliError {
    fn exit_code(&self) -> u8 {
        match self {
            Self::Resolve(ResolveError::SessionNotFound(_)) => 3,
            Self::ProviderNotFound(_) => 3,
            Self::RouteRuleNotFound(_) => 3,
            Self::Config(_) | Self::Resolve(ResolveError::EmptySessionId) => 2,
            Self::Index(_)
            | Self::Json(_)
            | Self::Output(_)
            | Self::ProviderStore(_)
            | Self::CcSwitchImport(_)
            | Self::RouteStartup(_)
            | Self::RouteServe(_)
            | Self::Lifecycle(_)
            | Self::Runtime(_) => 4,
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
            Self::ProviderStore(error) => error.fmt(formatter),
            Self::CcSwitchImport(error) => error.fmt(formatter),
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::ProviderNotFound(id) => write!(formatter, "provider '{id}' was not found"),
            Self::RouteRuleNotFound(path) => {
                write!(
                    formatter,
                    "workspace route was not found: {}",
                    path.display()
                )
            }
            Self::RouteStartup(error) => error.fmt(formatter),
            Self::RouteServe(error) => write!(formatter, "route server failed: {error}"),
            Self::Runtime(error) => write!(formatter, "failed to start route runtime: {error}"),
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

impl From<ProviderStoreError> for CliError {
    fn from(error: ProviderStoreError) -> Self {
        Self::ProviderStore(error)
    }
}

impl From<CcSwitchImportError> for CliError {
    fn from(error: CcSwitchImportError) -> Self {
        Self::CcSwitchImport(error)
    }
}

impl From<ResolveError> for CliError {
    fn from(error: ResolveError) -> Self {
        Self::Resolve(error)
    }
}

impl From<RouteStartupError> for CliError {
    fn from(error: RouteStartupError) -> Self {
        Self::RouteStartup(error)
    }
}

impl From<LifecycleError> for CliError {
    fn from(error: LifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}
