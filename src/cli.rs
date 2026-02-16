use std::io::{self, IsTerminal, Read};

use clap::Subcommand;
use serde::Serialize;

use crate::error::AtlasError;
use crate::models::{AtomType, Confidence};
use crate::tools::{
    delete_atom, enable_local_storage, get_atom, link, list_atoms, list_projects, search, unlink,
    upsert, DeleteAtomRequest, EnableLocalStorageRequest, GetAtomRequest, LinkRequest,
    ListAtomsRequest, SearchRequest, UpsertRequest,
};

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start MCP server (default when no subcommand)
    Serve {
        /// Run HTTP/SSE server on specified port
        #[arg(long)]
        http: Option<u16>,
    },

    /// Search atoms by query
    Search {
        /// Search query (use '-' for stdin)
        query: Option<String>,

        /// Filter by atom types
        #[arg(long = "type", short = 't')]
        types: Vec<AtomType>,

        /// Filter by tags
        #[arg(long, short = 'T')]
        tag: Vec<String>,

        /// Filter by confidence level
        #[arg(long, short = 'c')]
        confidence: Option<Confidence>,

        /// Page number (1-indexed)
        #[arg(long, short = 'p', default_value = "1")]
        page: usize,

        /// Results per page
        #[arg(long, short = 's', default_value = "20")]
        page_size: usize,

        /// Scope filter: org name or org/project path
        #[arg(long)]
        scope: Option<String>,
    },

    /// Get a single atom by ID
    Get {
        /// Atom ID (org/project/id, project/id, or bare id)
        id: String,
    },

    /// List atoms with optional filters
    Atoms {
        /// Filter by atom types
        #[arg(long = "type", short = 't')]
        types: Vec<AtomType>,

        /// Filter by tags
        #[arg(long, short = 'T')]
        tag: Vec<String>,

        /// Filter by confidence level
        #[arg(long, short = 'c')]
        confidence: Option<Confidence>,

        /// Maximum number of results
        #[arg(long, short = 'l', default_value = "1000")]
        limit: usize,

        /// Scope filter: org name or org/project path
        #[arg(long)]
        scope: Option<String>,
    },

    /// Create or update an atom
    Upsert {
        /// Atom ID for updates (omit for new atoms)
        #[arg(long)]
        id: Option<String>,

        /// Short descriptive title
        #[arg(long)]
        title: String,

        /// Type of knowledge
        #[arg(long = "type", short = 't')]
        atom_type: AtomType,

        /// Confidence level
        #[arg(long, short = 'c')]
        confidence: Confidence,

        /// Brief explanation
        #[arg(long)]
        summary: String,

        /// Extended content
        #[arg(long)]
        details: Option<String>,

        /// Potential pitfalls (can specify multiple)
        #[arg(long)]
        pitfall: Vec<String>,

        /// Keywords for search (can specify multiple)
        #[arg(long, short = 'T')]
        tag: Vec<String>,

        /// References (can specify multiple)
        #[arg(long)]
        source: Vec<String>,

        /// Related atoms (can specify multiple)
        #[arg(long)]
        link: Vec<String>,
    },

    /// Delete an atom
    Delete {
        /// Atom ID (org/project/id, project/id, or bare id)
        id: String,
    },

    /// Create a directed link between atoms
    Link {
        /// Source atom ID
        source: String,

        /// Target atom ID
        target: String,
    },

    /// Remove a directed link between atoms
    Unlink {
        /// Source atom ID
        source: String,

        /// Target atom ID
        target: String,
    },

    /// List all projects
    Projects,

    /// Show detected project context
    Context,

    /// Enable local storage for a project
    EnableLocal {
        /// Organization name
        #[arg(long)]
        org: String,

        /// Project name
        #[arg(long)]
        project: String,
    },
}

/// Run a CLI command and print output.
pub fn run(cmd: Commands, json: bool) -> anyhow::Result<()> {
    match cmd {
        Commands::Serve { .. } => {
            // Should not reach here - handled in main.rs
            unreachable!("Serve command should be handled in main.rs")
        }
        Commands::Search {
            query,
            types,
            tag,
            confidence,
            page,
            page_size,
            scope,
        } => {
            let query = resolve_input(query)?;
            let req = SearchRequest {
                query: if query.is_empty() { None } else { Some(query) },
                types: if types.is_empty() { None } else { Some(types) },
                tags: if tag.is_empty() { None } else { Some(tag) },
                confidence,
                page: Some(page),
                page_size: Some(page_size),
                scope,
            };
            let results = search(req)?;
            print_output(&results, json)?;
        }
        Commands::Get { id } => {
            let req = GetAtomRequest { id };
            let atom = get_atom(req)?;
            print_output(&atom, json)?;
        }
        Commands::Atoms {
            types,
            tag,
            confidence,
            limit,
            scope,
        } => {
            let req = ListAtomsRequest {
                types: if types.is_empty() { None } else { Some(types) },
                tags: if tag.is_empty() { None } else { Some(tag) },
                confidence,
                limit: Some(limit),
                scope,
            };
            let results = list_atoms(req)?;
            print_output(&results, json)?;
        }
        Commands::Upsert {
            id,
            title,
            atom_type,
            confidence,
            summary,
            details,
            pitfall,
            tag,
            source,
            link,
        } => {
            let req = UpsertRequest {
                id,
                title,
                atom_type,
                confidence,
                summary,
                details,
                pitfalls: if pitfall.is_empty() {
                    None
                } else {
                    Some(pitfall)
                },
                tags: if tag.is_empty() { None } else { Some(tag) },
                sources: if source.is_empty() {
                    None
                } else {
                    Some(source)
                },
                links: if link.is_empty() { None } else { Some(link) },
            };
            let result = upsert(req)?;
            print_output(&result, json)?;
        }
        Commands::Delete { id } => {
            let req = DeleteAtomRequest { id };
            let result = delete_atom(req)?;
            print_output(&result, json)?;
        }
        Commands::Link { source, target } => {
            let req = LinkRequest { source, target };
            let result = link(req)?;
            print_output(&result, json)?;
        }
        Commands::Unlink { source, target } => {
            let req = LinkRequest { source, target };
            let result = unlink(req)?;
            print_output(&result, json)?;
        }
        Commands::Projects => {
            let results = list_projects()?;
            print_output(&results, json)?;
        }
        Commands::Context => {
            let detected = crate::context::detect_context_full(None, None, None)?;
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            let info = ContextOutput {
                org: detected.context.org,
                project: detected.context.project,
                cwd,
                source: detected.source,
            };
            print_output(&info, json)?;
        }
        Commands::EnableLocal { org, project } => {
            let req = EnableLocalStorageRequest { org, project };
            let result = enable_local_storage(req)?;
            print_output(&result, json)?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct ContextOutput {
    org: String,
    project: String,
    cwd: String,
    source: crate::context::ContextSource,
}

/// Resolve input: if arg is Some("-") or None and stdin is piped, read from stdin.
fn resolve_input(arg: Option<String>) -> Result<String, AtlasError> {
    match arg {
        Some(s) if s == "-" => read_stdin(),
        Some(s) => Ok(s),
        None => {
            // No argument provided - check if stdin has data
            if !io::stdin().is_terminal() {
                read_stdin()
            } else {
                Ok(String::new())
            }
        }
    }
}

/// Read all input from stdin.
fn read_stdin() -> Result<String, AtlasError> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(AtlasError::Io)?;
    Ok(buffer.trim().to_string())
}

/// Print output as YAML (default) or JSON.
fn print_output<T: Serialize>(value: &T, json: bool) -> Result<(), AtlasError> {
    if json {
        let output = serde_json::to_string_pretty(value)
            .map_err(|e| AtlasError::Config(format!("JSON serialization error: {}", e)))?;
        println!("{}", output);
    } else {
        let output = serde_yaml::to_string(value)?;
        print!("{}", output);
    }
    Ok(())
}
