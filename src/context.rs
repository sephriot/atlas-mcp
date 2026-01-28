use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

use crate::error::AtlasError;

/// HTTP header for org override
pub const HEADER_ATLAS_ORG: &str = "x-atlas-org";
/// HTTP header for project override
pub const HEADER_ATLAS_PROJECT: &str = "x-atlas-project";

/// Detected project context.
#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub org: String,
    pub project: String,
}

impl ProjectContext {
    pub fn new(org: String, project: String) -> Self {
        Self { org, project }
    }
}

/// Detect project context from the current working directory.
///
/// Detection priority:
/// 1. ATLAS_ORG + ATLAS_PROJECT env vars (explicit configuration via CLI)
/// 2. Git remote URL parsing
/// 3. Fallback: global/{current_directory_name}
pub fn detect_context() -> Result<ProjectContext, AtlasError> {
    detect_context_with_headers(None, None)
}

/// Detect project context with optional HTTP header overrides.
///
/// Detection priority:
/// 1. HTTP headers (x-atlas-org, x-atlas-project) - for HTTP mode per-session context
/// 2. ATLAS_ORG + ATLAS_PROJECT env vars (explicit configuration via CLI)
/// 3. Git remote URL parsing
/// 4. Fallback: global/{current_directory_name}
pub fn detect_context_with_headers(
    header_org: Option<&str>,
    header_project: Option<&str>,
) -> Result<ProjectContext, AtlasError> {
    // Check for HTTP header overrides first (per-session in HTTP mode)
    if let (Some(org), Some(project)) = (header_org, header_project) {
        return Ok(ProjectContext::new(org.to_string(), project.to_string()));
    }

    // Check for explicit env var configuration (CLI args)
    if let (Ok(org), Ok(project)) = (std::env::var("ATLAS_ORG"), std::env::var("ATLAS_PROJECT")) {
        return Ok(ProjectContext::new(org, project));
    }

    let cwd = get_working_directory()?;
    detect_context_from_path(&cwd)
}

/// Get the working directory, respecting ATLAS_CWD env var.
fn get_working_directory() -> Result<PathBuf, AtlasError> {
    if let Ok(path) = std::env::var("ATLAS_CWD") {
        return Ok(PathBuf::from(path));
    }
    std::env::current_dir().map_err(|e| AtlasError::Context(format!("Failed to get CWD: {}", e)))
}

/// Detect project context from a specific path.
pub fn detect_context_from_path(path: &Path) -> Result<ProjectContext, AtlasError> {
    // Try git remote first
    if let Some(ctx) = try_git_remote(path) {
        return Ok(ctx);
    }

    // Fall back to global/<directory_name>
    let dir_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    Ok(ProjectContext::new(
        "global".to_string(),
        dir_name.to_string(),
    ))
}

/// Try to get org/project from git remote URL.
fn try_git_remote(path: &Path) -> Option<ProjectContext> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_git_url(&url)
}

/// Parse a git URL to extract org and project.
///
/// Supports:
/// - git@github.com:spacelift-io/worker.git
/// - https://github.com/sephriot/atlas-mcp.git
/// - git@gitlab.com:org/project.git
/// - https://gitlab.com/org/project
fn parse_git_url(url: &str) -> Option<ProjectContext> {
    // SSH format: git@host:org/project.git
    let ssh_re = Regex::new(r"^git@[^:]+:([^/]+)/([^/]+?)(?:\.git)?$").ok()?;
    if let Some(caps) = ssh_re.captures(url) {
        return Some(ProjectContext::new(
            caps.get(1)?.as_str().to_string(),
            caps.get(2)?.as_str().to_string(),
        ));
    }

    // HTTPS format: https://host/org/project.git
    let https_re = Regex::new(r"^https?://[^/]+/([^/]+)/([^/]+?)(?:\.git)?$").ok()?;
    if let Some(caps) = https_re.captures(url) {
        return Some(ProjectContext::new(
            caps.get(1)?.as_str().to_string(),
            caps.get(2)?.as_str().to_string(),
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_github() {
        let ctx = parse_git_url("git@github.com:spacelift-io/worker.git").unwrap();
        assert_eq!(ctx.org, "spacelift-io");
        assert_eq!(ctx.project, "worker");
    }

    #[test]
    fn test_parse_ssh_gitlab() {
        let ctx = parse_git_url("git@gitlab.com:myorg/myproject.git").unwrap();
        assert_eq!(ctx.org, "myorg");
        assert_eq!(ctx.project, "myproject");
    }

    #[test]
    fn test_parse_ssh_no_git_suffix() {
        let ctx = parse_git_url("git@github.com:org/repo").unwrap();
        assert_eq!(ctx.org, "org");
        assert_eq!(ctx.project, "repo");
    }

    #[test]
    fn test_parse_https_github() {
        let ctx = parse_git_url("https://github.com/sephriot/atlas-mcp.git").unwrap();
        assert_eq!(ctx.org, "sephriot");
        assert_eq!(ctx.project, "atlas-mcp");
    }

    #[test]
    fn test_parse_https_no_git_suffix() {
        let ctx = parse_git_url("https://github.com/sephriot/atlas-mcp").unwrap();
        assert_eq!(ctx.org, "sephriot");
        assert_eq!(ctx.project, "atlas-mcp");
    }

    #[test]
    fn test_parse_http_url() {
        let ctx = parse_git_url("http://gitlab.example.com/team/project.git").unwrap();
        assert_eq!(ctx.org, "team");
        assert_eq!(ctx.project, "project");
    }

    #[test]
    fn test_parse_invalid_url() {
        assert!(parse_git_url("not-a-url").is_none());
        assert!(parse_git_url("").is_none());
        assert!(parse_git_url("https://github.com/only-one-part").is_none());
    }

    #[test]
    fn test_project_context_new() {
        let ctx = ProjectContext::new("my-org".to_string(), "my-project".to_string());
        assert_eq!(ctx.org, "my-org");
        assert_eq!(ctx.project, "my-project");
    }

    #[test]
    fn test_detect_context_from_env_vars() {
        // Save original values
        let orig_org = std::env::var("ATLAS_ORG").ok();
        let orig_project = std::env::var("ATLAS_PROJECT").ok();

        // Set test values
        std::env::set_var("ATLAS_ORG", "test-org");
        std::env::set_var("ATLAS_PROJECT", "test-project");

        let ctx = detect_context().unwrap();
        assert_eq!(ctx.org, "test-org");
        assert_eq!(ctx.project, "test-project");

        // Restore original values
        match orig_org {
            Some(v) => std::env::set_var("ATLAS_ORG", v),
            None => std::env::remove_var("ATLAS_ORG"),
        }
        match orig_project {
            Some(v) => std::env::set_var("ATLAS_PROJECT", v),
            None => std::env::remove_var("ATLAS_PROJECT"),
        }
    }
}
