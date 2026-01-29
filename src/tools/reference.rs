//! Atom reference parsing and formatting utilities.
//!
//! Handles conversion between full path format (`org/project/K-000001`) and
//! context-relative formats (`project/K-000001` or `K-000001`).

use crate::context::ProjectContext;
use crate::error::AtlasError;

/// Parsed atom reference with org, project, and id components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomRef {
    pub org: String,
    pub project: String,
    pub id: String,
}

impl AtomRef {
    pub fn new(org: String, project: String, id: String) -> Self {
        Self { org, project, id }
    }

    /// Format as full path: "org/project/id"
    #[allow(dead_code)]
    pub fn to_full_path(&self) -> String {
        format!("{}/{}/{}", self.org, self.project, self.id)
    }

    /// Format relative to a project context.
    /// - Same project: "K-000001"
    /// - Cross-project: "other-project/K-000001"
    #[allow(dead_code)]
    pub fn to_relative(&self, ctx: &ProjectContext) -> String {
        if self.org == ctx.org && self.project == ctx.project {
            self.id.clone()
        } else if self.org == ctx.org {
            format!("{}/{}", self.project, self.id)
        } else {
            self.to_full_path()
        }
    }
}

/// Parse atom reference, filling missing parts from context.
///
/// Accepts:
/// - "org/project/K-000001" (full path)
/// - "project/K-000001" (project-qualified, uses context org)
/// - "K-000001" (bare id, uses context org and project)
pub fn parse_atom_reference(ref_str: &str, ctx: &ProjectContext) -> AtomRef {
    let parts: Vec<&str> = ref_str.split('/').collect();
    match parts.len() {
        3 => AtomRef::new(
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
        ),
        2 => AtomRef::new(ctx.org.clone(), parts[0].to_string(), parts[1].to_string()),
        _ => AtomRef::new(ctx.org.clone(), ctx.project.clone(), ref_str.to_string()),
    }
}

/// Format full atom reference: "org/project/id"
pub fn format_atom_reference(org: &str, project: &str, id: &str) -> String {
    format!("{}/{}/{}", org, project, id)
}

/// Common placeholder patterns that AI models might use literally.
const PLACEHOLDER_PATTERNS: &[&str] = &[
    "org",
    "project",
    "org/project",
    "<org>",
    "<project>",
    "<org>/<project>",
    "{org}",
    "{project}",
    "{org}/{project}",
];

/// Parse scope filter for search/list operations.
///
/// Accepts:
/// - None -> use detected context (search entire org)
/// - "org" -> search all projects in that org
/// - "org/project" -> search only that project
///
/// Returns (org, optional_project) or error if scope is a placeholder.
pub fn parse_scope(
    scope: Option<&str>,
    ctx: &ProjectContext,
) -> Result<(String, Option<String>), AtlasError> {
    match scope {
        None => Ok((ctx.org.clone(), None)),
        Some(s) => {
            // Reject common placeholder patterns
            if PLACEHOLDER_PATTERNS.contains(&s.to_lowercase().as_str()) {
                return Err(AtlasError::Validation(format!(
                    "Invalid scope '{}'. Expected actual org/project names, e.g., 'acme' or 'acme/backend'. \
                    Use get_context to find your current org/project.",
                    s
                )));
            }

            let parts: Vec<&str> = s.split('/').collect();
            match parts.len() {
                1 => Ok((parts[0].to_string(), None)),
                2 => Ok((parts[0].to_string(), Some(parts[1].to_string()))),
                _ => Ok((ctx.org.clone(), None)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> ProjectContext {
        ProjectContext::new("test-org".to_string(), "test-project".to_string())
    }

    #[test]
    fn test_parse_full_path() {
        let ctx = test_ctx();
        let r = parse_atom_reference("other-org/other-project/K-000001", &ctx);
        assert_eq!(r.org, "other-org");
        assert_eq!(r.project, "other-project");
        assert_eq!(r.id, "K-000001");
    }

    #[test]
    fn test_parse_project_qualified() {
        let ctx = test_ctx();
        let r = parse_atom_reference("other-project/K-000001", &ctx);
        assert_eq!(r.org, "test-org");
        assert_eq!(r.project, "other-project");
        assert_eq!(r.id, "K-000001");
    }

    #[test]
    fn test_parse_bare_id() {
        let ctx = test_ctx();
        let r = parse_atom_reference("K-000001", &ctx);
        assert_eq!(r.org, "test-org");
        assert_eq!(r.project, "test-project");
        assert_eq!(r.id, "K-000001");
    }

    #[test]
    fn test_to_full_path() {
        let r = AtomRef::new(
            "org".to_string(),
            "proj".to_string(),
            "K-000001".to_string(),
        );
        assert_eq!(r.to_full_path(), "org/proj/K-000001");
    }

    #[test]
    fn test_to_relative_same_project() {
        let ctx = test_ctx();
        let r = AtomRef::new(
            "test-org".to_string(),
            "test-project".to_string(),
            "K-000001".to_string(),
        );
        assert_eq!(r.to_relative(&ctx), "K-000001");
    }

    #[test]
    fn test_to_relative_cross_project() {
        let ctx = test_ctx();
        let r = AtomRef::new(
            "test-org".to_string(),
            "other-project".to_string(),
            "K-000001".to_string(),
        );
        assert_eq!(r.to_relative(&ctx), "other-project/K-000001");
    }

    #[test]
    fn test_to_relative_cross_org() {
        let ctx = test_ctx();
        let r = AtomRef::new(
            "other-org".to_string(),
            "other-project".to_string(),
            "K-000001".to_string(),
        );
        assert_eq!(r.to_relative(&ctx), "other-org/other-project/K-000001");
    }

    #[test]
    fn test_format_atom_reference() {
        assert_eq!(
            format_atom_reference("org", "proj", "K-000001"),
            "org/proj/K-000001"
        );
    }

    #[test]
    fn test_parse_scope_none() {
        let ctx = test_ctx();
        let (org, proj) = parse_scope(None, &ctx).unwrap();
        assert_eq!(org, "test-org");
        assert_eq!(proj, None);
    }

    #[test]
    fn test_parse_scope_org_only() {
        let ctx = test_ctx();
        let (org, proj) = parse_scope(Some("other-org"), &ctx).unwrap();
        assert_eq!(org, "other-org");
        assert_eq!(proj, None);
    }

    #[test]
    fn test_parse_scope_org_and_project() {
        let ctx = test_ctx();
        let (org, proj) = parse_scope(Some("other-org/other-project"), &ctx).unwrap();
        assert_eq!(org, "other-org");
        assert_eq!(proj, Some("other-project".to_string()));
    }

    #[test]
    fn test_parse_scope_rejects_placeholder_org_project() {
        let ctx = test_ctx();
        let result = parse_scope(Some("org/project"), &ctx);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid scope"));
        assert!(err.contains("org/project"));
    }

    #[test]
    fn test_parse_scope_rejects_placeholder_org() {
        let ctx = test_ctx();
        let result = parse_scope(Some("org"), &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_scope_rejects_placeholder_project() {
        let ctx = test_ctx();
        let result = parse_scope(Some("project"), &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_scope_rejects_bracketed_placeholders() {
        let ctx = test_ctx();
        assert!(parse_scope(Some("<org>"), &ctx).is_err());
        assert!(parse_scope(Some("<project>"), &ctx).is_err());
        assert!(parse_scope(Some("<org>/<project>"), &ctx).is_err());
        assert!(parse_scope(Some("{org}"), &ctx).is_err());
        assert!(parse_scope(Some("{project}"), &ctx).is_err());
        assert!(parse_scope(Some("{org}/{project}"), &ctx).is_err());
    }

    #[test]
    fn test_parse_scope_case_insensitive_placeholder_detection() {
        let ctx = test_ctx();
        assert!(parse_scope(Some("ORG"), &ctx).is_err());
        assert!(parse_scope(Some("PROJECT"), &ctx).is_err());
        assert!(parse_scope(Some("Org/Project"), &ctx).is_err());
    }
}
