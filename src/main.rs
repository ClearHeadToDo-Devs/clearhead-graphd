use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clearhead_core::WorkspaceConfig;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "clearhead-graphd")]
#[command(about = "Standalone graph/query binary for ClearHead")]
struct Cli {
    /// Workspace root to load.
    #[arg(short, long, default_value = ".")]
    workspace: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Execute a one-shot graph query.
    Query,

    /// Convert a JSON-encoded domain model from stdin to canonical JSON-LD.
    ExportJsonld,
}

/// Versioned request read from stdin. Keeping the SPARQL out of argv avoids
/// command-line size and quoting constraints, while the version gives clients
/// a concrete compatibility check as this process boundary grows.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryRequest {
    version: u32,
    sparql: String,
    #[serde(default)]
    config: GraphConfig,
    #[serde(default)]
    output: QueryOutput,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum QueryOutput {
    #[default]
    Rows,
    IndexJsonld,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphConfig {
    #[serde(default)]
    tag_hierarchies: HashMap<String, Vec<String>>,
    #[serde(default)]
    additional_workspaces: Vec<String>,
}

impl From<GraphConfig> for WorkspaceConfig {
    fn from(config: GraphConfig) -> Self {
        Self {
            tag_hierarchies: config.tag_hierarchies,
            additional_workspaces: config.additional_workspaces,
            ..Self::default()
        }
    }
}

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    match cli.command {
        Command::Query => run_query(&cli.workspace, std::io::stdin()),
        Command::ExportJsonld => export_jsonld(std::io::stdin()),
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .try_init();
}

fn run_query(workspace: &Path, mut input: impl Read) -> Result<()> {
    let mut request_json = String::new();
    input
        .read_to_string(&mut request_json)
        .context("Failed to read query request from stdin")?;
    let request: QueryRequest =
        serde_json::from_str(&request_json).context("Invalid graphd query request")?;
    if request.version != 1 {
        anyhow::bail!(
            "unsupported graphd query request version {}; expected 1",
            request.version
        );
    }
    if request.sparql.trim().is_empty() {
        anyhow::bail!("graphd query request contains empty SPARQL");
    }

    let config = WorkspaceConfig::from(request.config);
    let rows = run_workspace_raw_query(workspace, &request.sparql, &config)?;
    let response = match request.output {
        QueryOutput::Rows => {
            serde_json::to_value(rows).context("Failed to serialize query rows")?
        }
        QueryOutput::IndexJsonld => clearhead_graphd::graph::frame_index(&rows)
            .context("Query result does not satisfy the index contract")?,
    };
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

fn export_jsonld(mut input: impl Read) -> Result<()> {
    let mut model_json = String::new();
    input
        .read_to_string(&mut model_json)
        .context("Failed to read domain model from stdin")?;
    let model: clearhead_core::DomainModel =
        serde_json::from_str(&model_json).context("Invalid domain model JSON")?;
    let jsonld = clearhead_graphd::graph::serialize_domain_to_jsonld(&model)
        .context("Failed to serialize JSON-LD")?;
    println!("{jsonld}");
    Ok(())
}

fn workspace_graph_name(
    workspace: &clearhead_core::workspace::store::load::Workspace,
) -> clearhead_graphd::graph::GraphName {
    clearhead_graphd::graph::GraphName::NamedNode(clearhead_graphd::graph::workspace_graph_uri(
        &workspace.effective_id(),
    ))
}

fn load_workspace_at_path_into_store(
    store: &clearhead_graphd::graph::Store,
    workspace_path: &Path,
) -> Result<()> {
    if !workspace_path.exists() {
        anyhow::bail!(
            "Additional workspace path does not exist: {}",
            workspace_path.display()
        );
    }

    let workspace = clearhead_core::workspace::store::load::Workspace::load(workspace_path)
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to load workspace at {}: {}",
                workspace_path.display(),
                e
            )
        })?;
    let graph_name = workspace_graph_name(&workspace);

    clearhead_graphd::graph::insert_workspace_metadata(store, &workspace, graph_name.clone())
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to insert workspace metadata for {}: {}",
                workspace_path.display(),
                e
            )
        })?;
    let model = clearhead_core::DomainModel::from(workspace);
    clearhead_graphd::graph::load_domain_model(store, &model, None, graph_name).map_err(|e| {
        anyhow::anyhow!(
            "Failed to insert workspace {} into store: {}",
            workspace_path.display(),
            e
        )
    })?;

    Ok(())
}

fn run_workspace_raw_query(
    data_dir: &Path,
    sparql: &str,
    config: &WorkspaceConfig,
) -> Result<Vec<HashMap<String, String>>> {
    let store = clearhead_graphd::graph::create_store()
        .map_err(|e| anyhow::anyhow!("Failed to create store: {}", e))?;

    let primary = clearhead_core::workspace::store::load::Workspace::load(data_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load workspace: {}", e))?;
    let graph_name = workspace_graph_name(&primary);

    clearhead_graphd::graph::insert_workspace_metadata(&store, &primary, graph_name.clone())
        .map_err(|e| anyhow::anyhow!("Failed to insert workspace metadata: {}", e))?;
    let model = clearhead_core::DomainModel::from(primary);
    clearhead_graphd::graph::load_domain_model(&store, &model, Some(config), graph_name)
        .map_err(|e| anyhow::anyhow!("Failed to load domain model: {}", e))?;

    for path_str in &config.additional_workspaces {
        let path = Path::new(path_str);
        if let Err(e) = load_workspace_at_path_into_store(&store, path) {
            tracing::warn!("Skipping additional workspace '{}': {e}", path_str);
        }
    }

    clearhead_graphd::graph::query_raw(&store, sparql)
        .map_err(|e| anyhow::anyhow!("SPARQL query failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_contract_version_before_loading_workspace() {
        let request = br#"{"version":2,"sparql":"SELECT * WHERE {}"}"#;
        let error = run_query(Path::new("does-not-matter"), &request[..]).unwrap_err();
        assert!(error.to_string().contains("expected 1"), "{error:#}");
    }

    #[test]
    fn rejects_unknown_request_fields() {
        let request = br#"{"version":1,"sparql":"SELECT * WHERE {}","format":"table"}"#;
        let error = run_query(Path::new("does-not-matter"), &request[..]).unwrap_err();
        assert!(
            error.to_string().contains("Invalid graphd query request"),
            "{error:#}"
        );
    }
}
