use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use schemars::JsonSchema;
use serde::Serialize;

use crate::error::AtlasError;

/// HTTP header for org override
pub const HEADER_ATLAS_ORG: &str = "x-atlas-org";
/// HTTP header for project override
pub const HEADER_ATLAS_PROJECT: &str = "x-atlas-project";

/// Source of detected context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    /// Explicit per-request override via HTTP headers
    HttpHeaders,
    /// Session-level override via activate_project
    Activated,
    /// Detected .atlas/ in current working directory
    LocalStorage,
    /// Automatic detection from git remote URL
    GitRemote,
    /// Explicit configuration via ATLAS_ORG/ATLAS_PROJECT env vars
    EnvVars,
    /// Fallback: global/{dirname}
    Fallback,
}

/// Detected context with source tracking.
#[derive(Debug, Clone)]
pub struct DetectedContext {
    pub context: ProjectContext,
    pub source: ContextSource,
}

impl DetectedContext {
    pub fn new(context: ProjectContext, source: ContextSource) -> Self {
        Self { context, source }
    }

    /// Returns true if context was detected via fallback.
    #[allow(dead_code)]
    pub fn is_fallback(&self) -> bool {
        matches!(self.source, ContextSource::Fallback)
    }
}

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

/// Validate org/project name.
///
/// Names must:
/// - Start with alphanumeric character
/// - Contain only alphanumeric, hyphens, and underscores
/// - Not be empty
pub fn validate_name(name: &str) -> Result<(), AtlasError> {
    if name.is_empty() {
        return Err(AtlasError::Validation("Name cannot be empty".to_string()));
    }

    let re = Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9_-]*$").unwrap();
    if !re.is_match(name) {
        return Err(AtlasError::Validation(format!(
            "Invalid name '{}': must start with alphanumeric and contain only alphanumeric, hyphens, underscores",
            name
        )));
    }

    Ok(())
}

/// Detect project context with optional HTTP header overrides.
///
/// Detection priority:
/// 1. HTTP headers (x-atlas-org, x-atlas-project) - for HTTP mode per-session context
/// 2. ATLAS_ORG + ATLAS_PROJECT env vars (explicit configuration via CLI)
/// 3. Git remote URL parsing
/// 4. Fallback: global/{current_directory_name}
#[allow(dead_code)]
pub fn detect_context_with_headers(
    header_org: Option<&str>,
    header_project: Option<&str>,
) -> Result<ProjectContext, AtlasError> {
    Ok(detect_context_full(header_org, header_project, None)?.context)
}

/// Detect project context with activation support and source tracking.
///
/// Detection priority:
/// 1. HTTP headers (x-atlas-org, x-atlas-project) - explicit per-request override
/// 2. Activated context - session-level override (user control)
/// 3. Local storage (.atlas/ in CWD)
/// 4. Git remote URL parsing
/// 5. Env vars (ATLAS_ORG/ATLAS_PROJECT)
/// 6. Fallback: global/{dirname}
#[allow(dead_code)]
pub fn detect_context_with_activation(
    activated: Option<&ProjectContext>,
) -> Result<DetectedContext, AtlasError> {
    detect_context_full(None, None, activated)
}

/// Full context detection with all override sources and tracking.
///
/// Detection priority:
/// 1. HTTP headers (x-atlas-org, x-atlas-project) - explicit per-request override
/// 2. Activated context - session-level override (user control)
/// 3. Local storage (.atlas/ in CWD)
/// 4. Git remote URL parsing
/// 5. Env vars (ATLAS_ORG/ATLAS_PROJECT)
/// 6. Fallback: global/{dirname}
pub fn detect_context_full(
    header_org: Option<&str>,
    header_project: Option<&str>,
    activated: Option<&ProjectContext>,
) -> Result<DetectedContext, AtlasError> {
    // 1. HTTP headers (explicit per-request override)
    if let (Some(org), Some(project)) = (header_org, header_project) {
        return Ok(DetectedContext::new(
            ProjectContext::new(org.to_string(), project.to_string()),
            ContextSource::HttpHeaders,
        ));
    }

    // 2. Activated context (session-level override)
    if let Some(ctx) = activated {
        return Ok(DetectedContext::new(ctx.clone(), ContextSource::Activated));
    }

    let cwd = get_working_directory()?;

    // 3. Local storage (.atlas/ in CWD)
    if let Some(ctx) = try_local_storage(&cwd) {
        return Ok(DetectedContext::new(ctx, ContextSource::LocalStorage));
    }

    // 4. Git remote
    if let Some(ctx) = try_git_remote(&cwd) {
        return Ok(DetectedContext::new(ctx, ContextSource::GitRemote));
    }

    // 5. Env vars
    if let (Ok(org), Ok(project)) = (std::env::var("ATLAS_ORG"), std::env::var("ATLAS_PROJECT")) {
        return Ok(DetectedContext::new(
            ProjectContext::new(org, project),
            ContextSource::EnvVars,
        ));
    }

    // 6. Fallback
    let dir_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    Ok(DetectedContext::new(
        ProjectContext::new("global".to_string(), dir_name.to_string()),
        ContextSource::Fallback,
    ))
}

/// Try to detect context from .atlas/ directory in CWD.
///
/// Looks for .atlas/config.yaml with org/project fields.
fn try_local_storage(path: &Path) -> Option<ProjectContext> {
    let atlas_dir = path.join(".atlas");
    if !atlas_dir.is_dir() {
        return None;
    }

    // Check for config.yaml with org/project
    let config_path = atlas_dir.join("config.yaml");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            // Simple YAML parsing for org/project
            let mut org: Option<String> = None;
            let mut project: Option<String> = None;

            for line in content.lines() {
                let line = line.trim();
                if let Some(value) = line.strip_prefix("org:") {
                    org = Some(
                        value
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string(),
                    );
                } else if let Some(value) = line.strip_prefix("project:") {
                    project = Some(
                        value
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string(),
                    );
                }
            }

            if let (Some(o), Some(p)) = (org, project) {
                if !o.is_empty() && !p.is_empty() {
                    return Some(ProjectContext::new(o, p));
                }
            }
        }
    }

    None
}

/// Get the working directory, respecting ATLAS_CWD env var.
fn get_working_directory() -> Result<PathBuf, AtlasError> {
    if let Ok(path) = std::env::var("ATLAS_CWD") {
        return Ok(PathBuf::from(path));
    }
    std::env::current_dir().map_err(|e| AtlasError::Context(format!("Failed to get CWD: {}", e)))
}

/// Detect project context from a specific path.
#[allow(dead_code)]
pub fn detect_context_from_path(path: &Path) -> Result<ProjectContext, AtlasError> {
    // Try local storage first
    if let Some(ctx) = try_local_storage(path) {
        return Ok(ctx);
    }

    // Try git remote
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
    fn test_detect_context_env_vars_priority() {
        // Note: With the new detection priority, git remote takes priority over env vars.
        // This test verifies env vars work when git detection is skipped
        // (i.e., when running the full detection chain in a non-git directory).

        // Save original values
        let orig_org = std::env::var("ATLAS_ORG").ok();
        let orig_project = std::env::var("ATLAS_PROJECT").ok();
        let orig_cwd = std::env::var("ATLAS_CWD").ok();

        // Set test values and point to a non-git directory
        std::env::set_var("ATLAS_ORG", "test-org");
        std::env::set_var("ATLAS_PROJECT", "test-project");
        std::env::set_var("ATLAS_CWD", "/tmp");

        let detected = detect_context_full(None, None, None).unwrap();
        assert_eq!(detected.context.org, "test-org");
        assert_eq!(detected.context.project, "test-project");
        assert_eq!(detected.source, ContextSource::EnvVars);

        // Restore original values
        match orig_org {
            Some(v) => std::env::set_var("ATLAS_ORG", v),
            None => std::env::remove_var("ATLAS_ORG"),
        }
        match orig_project {
            Some(v) => std::env::set_var("ATLAS_PROJECT", v),
            None => std::env::remove_var("ATLAS_PROJECT"),
        }
        match orig_cwd {
            Some(v) => std::env::set_var("ATLAS_CWD", v),
            None => std::env::remove_var("ATLAS_CWD"),
        }
    }

    // ============================================================================
    // ContextSource tests
    // ============================================================================

    #[test]
    fn test_context_source_serialization() {
        assert_eq!(
            serde_json::to_string(&ContextSource::HttpHeaders).unwrap(),
            r#""http_headers""#
        );
        assert_eq!(
            serde_json::to_string(&ContextSource::Activated).unwrap(),
            r#""activated""#
        );
        assert_eq!(
            serde_json::to_string(&ContextSource::LocalStorage).unwrap(),
            r#""local_storage""#
        );
        assert_eq!(
            serde_json::to_string(&ContextSource::GitRemote).unwrap(),
            r#""git_remote""#
        );
        assert_eq!(
            serde_json::to_string(&ContextSource::EnvVars).unwrap(),
            r#""env_vars""#
        );
        assert_eq!(
            serde_json::to_string(&ContextSource::Fallback).unwrap(),
            r#""fallback""#
        );
    }

    // ============================================================================
    // validate_name tests
    // ============================================================================

    #[test]
    fn test_validate_name_valid() {
        assert!(validate_name("myorg").is_ok());
        assert!(validate_name("my-org").is_ok());
        assert!(validate_name("my_org").is_ok());
        assert!(validate_name("MyOrg123").is_ok());
        assert!(validate_name("a").is_ok());
        assert!(validate_name("1test").is_ok());
    }

    #[test]
    fn test_validate_name_invalid() {
        assert!(validate_name("").is_err());
        assert!(validate_name("-invalid").is_err());
        assert!(validate_name("_invalid").is_err());
        assert!(validate_name("has space").is_err());
        assert!(validate_name("has/slash").is_err());
        assert!(validate_name("has.dot").is_err());
        assert!(validate_name("../traversal").is_err());
    }

    // ============================================================================
    // DetectedContext tests
    // ============================================================================

    #[test]
    fn test_detected_context_is_fallback() {
        let fallback = DetectedContext::new(
            ProjectContext::new("global".into(), "test".into()),
            ContextSource::Fallback,
        );
        assert!(fallback.is_fallback());

        let git = DetectedContext::new(
            ProjectContext::new("org".into(), "proj".into()),
            ContextSource::GitRemote,
        );
        assert!(!git.is_fallback());

        let activated = DetectedContext::new(
            ProjectContext::new("org".into(), "proj".into()),
            ContextSource::Activated,
        );
        assert!(!activated.is_fallback());
    }

    // ============================================================================
    // detect_context_full tests
    // ============================================================================

    #[test]
    fn test_detect_context_full_http_headers_highest_priority() {
        let activated = ProjectContext::new("activated-org".into(), "activated-proj".into());
        let result =
            detect_context_full(Some("header-org"), Some("header-proj"), Some(&activated)).unwrap();

        assert_eq!(result.context.org, "header-org");
        assert_eq!(result.context.project, "header-proj");
        assert_eq!(result.source, ContextSource::HttpHeaders);
    }

    #[test]
    fn test_detect_context_full_activated_second_priority() {
        let activated = ProjectContext::new("activated-org".into(), "activated-proj".into());
        let result = detect_context_full(None, None, Some(&activated)).unwrap();

        assert_eq!(result.context.org, "activated-org");
        assert_eq!(result.context.project, "activated-proj");
        assert_eq!(result.source, ContextSource::Activated);
    }

    #[test]
    fn test_detect_context_with_activation() {
        let activated = ProjectContext::new("test-org".into(), "test-proj".into());
        let result = detect_context_with_activation(Some(&activated)).unwrap();

        assert_eq!(result.context.org, "test-org");
        assert_eq!(result.context.project, "test-proj");
        assert_eq!(result.source, ContextSource::Activated);
    }
}
