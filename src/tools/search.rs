use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::{detect_context, ProjectContext};
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{AtomType, Confidence, IndexEntry};
use crate::storage::load_index;

/// Search request parameters.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SearchRequest {
    /// Search query - matches against title and tags
    #[serde(default)]
    pub query: Option<String>,

    /// Filter by atom types
    #[serde(default)]
    pub types: Option<Vec<AtomType>>,

    /// Filter by tags (matches any)
    #[serde(default)]
    pub tags: Option<Vec<String>>,

    /// Filter by confidence level
    #[serde(default)]
    pub confidence: Option<Confidence>,

    /// Maximum number of results (default: 20)
    #[serde(default)]
    pub limit: Option<usize>,

    /// Override org (defaults to detected)
    #[serde(default)]
    pub org: Option<String>,

    /// Override project (defaults to detected)
    #[serde(default)]
    pub project: Option<String>,
}

/// Search result entry.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub atom_type: AtomType,
    pub confidence: Confidence,
    pub tags: Vec<String>,
    pub score: f32,
}

impl From<(&IndexEntry, f32)> for SearchResult {
    fn from((entry, score): (&IndexEntry, f32)) -> Self {
        Self {
            id: entry.id.clone(),
            title: entry.title.clone(),
            atom_type: entry.atom_type,
            confidence: entry.confidence,
            tags: entry.tags.clone(),
            score,
        }
    }
}

/// Search atoms in the index.
pub fn search(req: SearchRequest) -> Result<Vec<SearchResult>, AtlasError> {
    let ctx = get_context_or_override(&req.org, &req.project)?;
    let _lock = ProjectLock::acquire(&ctx.org, &ctx.project)?;
    let index = load_index(&ctx.org, &ctx.project)?;

    let limit = req.limit.unwrap_or(20);
    let query_terms = req
        .query
        .as_ref()
        .map(|q| {
            q.to_lowercase()
                .split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut results: Vec<(SearchResult, f32)> = index
        .entries
        .iter()
        .filter_map(|entry| {
            // Apply type filter
            if let Some(ref types) = req.types {
                if !types.contains(&entry.atom_type) {
                    return None;
                }
            }

            // Apply confidence filter
            if let Some(ref conf) = req.confidence {
                if &entry.confidence != conf {
                    return None;
                }
            }

            // Apply tags filter (match any)
            if let Some(ref filter_tags) = req.tags {
                let entry_tags_lower: Vec<String> =
                    entry.tags.iter().map(|t| t.to_lowercase()).collect();
                let has_match = filter_tags
                    .iter()
                    .any(|t| entry_tags_lower.contains(&t.to_lowercase()));
                if !has_match {
                    return None;
                }
            }

            // Calculate score
            let score = calculate_score(entry, &query_terms);

            // If query provided, require minimum score
            if !query_terms.is_empty() && score == 0.0 {
                return None;
            }

            Some((SearchResult::from((entry, score)), score))
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Apply limit
    let results: Vec<SearchResult> = results.into_iter().take(limit).map(|(r, _)| r).collect();

    Ok(results)
}

fn calculate_score(entry: &IndexEntry, query_terms: &[String]) -> f32 {
    if query_terms.is_empty() {
        return 1.0; // No query means match all
    }

    let title_lower = entry.title.to_lowercase();
    let tags_lower: Vec<String> = entry.tags.iter().map(|t| t.to_lowercase()).collect();

    let mut score = 0.0;

    for term in query_terms {
        // Title exact match
        if title_lower == *term {
            score += 10.0;
        }
        // Title contains
        else if title_lower.contains(term) {
            score += 5.0;
        }

        // Tag exact match
        if tags_lower.contains(term) {
            score += 3.0;
        }

        // Tag contains
        for tag in &tags_lower {
            if tag.contains(term) && tag != term {
                score += 1.0;
            }
        }
    }

    score
}

fn get_context_or_override(
    org: &Option<String>,
    project: &Option<String>,
) -> Result<ProjectContext, AtlasError> {
    match (org, project) {
        (Some(o), Some(p)) => Ok(ProjectContext::new(o.clone(), p.clone())),
        (Some(_), None) | (None, Some(_)) => Err(AtlasError::Validation(
            "Both org and project must be specified together".into(),
        )),
        (None, None) => detect_context(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, title: &str, tags: Vec<&str>) -> IndexEntry {
        IndexEntry {
            id: id.to_string(),
            title: title.to_string(),
            atom_type: AtomType::Note,
            confidence: Confidence::High,
            tags: tags.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_calculate_score_empty_query() {
        let entry = make_entry("K-000001", "Test", vec![]);
        let score = calculate_score(&entry, &[]);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn test_calculate_score_title_exact_match() {
        let entry = make_entry("K-000001", "error", vec![]);
        let terms = vec!["error".to_string()];
        let score = calculate_score(&entry, &terms);
        assert_eq!(score, 10.0);
    }

    #[test]
    fn test_calculate_score_title_contains() {
        let entry = make_entry("K-000001", "Error Handling Pattern", vec![]);
        let terms = vec!["error".to_string()];
        let score = calculate_score(&entry, &terms);
        assert_eq!(score, 5.0);
    }

    #[test]
    fn test_calculate_score_tag_exact_match() {
        let entry = make_entry("K-000001", "Something", vec!["rust", "async"]);
        let terms = vec!["rust".to_string()];
        let score = calculate_score(&entry, &terms);
        assert_eq!(score, 3.0);
    }

    #[test]
    fn test_calculate_score_tag_contains() {
        let entry = make_entry("K-000001", "Something", vec!["error-handling"]);
        let terms = vec!["error".to_string()];
        let score = calculate_score(&entry, &terms);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn test_calculate_score_combined() {
        let entry = make_entry("K-000001", "Error Handling", vec!["error", "rust"]);
        let terms = vec!["error".to_string()];
        // Title contains "error" = 5.0, tag exact match "error" = 3.0
        let score = calculate_score(&entry, &terms);
        assert_eq!(score, 8.0);
    }

    #[test]
    fn test_calculate_score_multiple_terms() {
        let entry = make_entry("K-000001", "Rust Error Handling", vec!["patterns"]);
        let terms = vec!["rust".to_string(), "error".to_string()];
        // "rust" in title = 5.0, "error" in title = 5.0
        let score = calculate_score(&entry, &terms);
        assert_eq!(score, 10.0);
    }

    #[test]
    fn test_calculate_score_case_insensitive() {
        let entry = make_entry("K-000001", "ERROR HANDLING", vec!["RUST"]);
        let terms = vec!["error".to_string(), "rust".to_string()];
        let score = calculate_score(&entry, &terms);
        assert!(score > 0.0);
    }

    #[test]
    fn test_calculate_score_no_match() {
        let entry = make_entry("K-000001", "Database Connection", vec!["sql"]);
        let terms = vec!["error".to_string()];
        let score = calculate_score(&entry, &terms);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_search_result_from_entry() {
        let entry = make_entry("K-000001", "Test", vec!["tag1"]);
        let result = SearchResult::from((&entry, 5.0));
        assert_eq!(result.id, "K-000001");
        assert_eq!(result.title, "Test");
        assert_eq!(result.score, 5.0);
    }
}
