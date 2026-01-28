use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;

use crate::config::get_orgs_path;
use crate::error::AtlasError;

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
pub fn detect_context() -> Result<ProjectContext, AtlasError> {
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

    // Try .knowledge symlink
    if let Some(ctx) = try_knowledge_symlink(path)? {
        return Ok(ctx);
    }

    // Fall back to global/<directory_name>
    let dir_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    Ok(ProjectContext::new("global".to_string(), dir_name.to_string()))
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

/// Try to resolve context from .knowledge symlink.
fn try_knowledge_symlink(path: &Path) -> Result<Option<ProjectContext>, AtlasError> {
    let symlink_path = path.join(".knowledge");

    if !symlink_path.is_symlink() {
        return Ok(None);
    }

    let target = std::fs::read_link(&symlink_path)?;
    let orgs_path = get_orgs_path()?;

    // Check if target is under ~/.atlas/orgs/{org}/{project}
    if let Ok(rel) = target.strip_prefix(&orgs_path) {
        let components: Vec<_> = rel.components().collect();
        if components.len() >= 2 {
            let org = components[0].as_os_str().to_string_lossy().to_string();
            let project = components[1].as_os_str().to_string_lossy().to_string();
            return Ok(Some(ProjectContext::new(org, project)));
        }
    }

    Ok(None)
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
}
