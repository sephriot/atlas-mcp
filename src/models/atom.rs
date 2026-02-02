use chrono::NaiveDate;
use clap::ValueEnum;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum AtomType {
    Note,
    Gotcha,
    Recipe,
    Decision,
}

impl std::fmt::Display for AtomType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AtomType::Note => write!(f, "note"),
            AtomType::Gotcha => write!(f, "gotcha"),
            AtomType::Recipe => write!(f, "recipe"),
            AtomType::Decision => write!(f, "decision"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::High => write!(f, "high"),
            Confidence::Medium => write!(f, "medium"),
            Confidence::Low => write!(f, "low"),
        }
    }
}

/// A knowledge atom representing a single piece of knowledge.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Atom {
    /// Unique identifier (e.g., K-000001)
    pub id: String,

    /// Short descriptive title
    pub title: String,

    /// Type of knowledge
    #[serde(rename = "type")]
    pub atom_type: AtomType,

    /// Confidence level
    pub confidence: Confidence,

    /// Brief explanation
    pub summary: String,

    /// Extended content (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    /// Potential pitfalls (optional)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pitfalls: Vec<String>,

    /// Keywords for search
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// References - simple strings (paths or URLs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,

    /// Related atoms - id or project/id format
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,

    /// Last update date
    pub updated_at: NaiveDate,
}

impl Atom {
    pub fn new(
        id: String,
        title: String,
        atom_type: AtomType,
        confidence: Confidence,
        summary: String,
    ) -> Self {
        Self {
            id,
            title,
            atom_type,
            confidence,
            summary,
            details: None,
            pitfalls: Vec::new(),
            tags: Vec::new(),
            sources: Vec::new(),
            links: Vec::new(),
            updated_at: chrono::Utc::now().date_naive(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atom_type_display() {
        assert_eq!(AtomType::Note.to_string(), "note");
        assert_eq!(AtomType::Gotcha.to_string(), "gotcha");
        assert_eq!(AtomType::Recipe.to_string(), "recipe");
        assert_eq!(AtomType::Decision.to_string(), "decision");
    }

    #[test]
    fn test_atom_type_serialization() {
        assert_eq!(serde_json::to_string(&AtomType::Note).unwrap(), "\"note\"");
        assert_eq!(
            serde_json::to_string(&AtomType::Gotcha).unwrap(),
            "\"gotcha\""
        );
    }

    #[test]
    fn test_atom_type_deserialization() {
        let note: AtomType = serde_json::from_str("\"note\"").unwrap();
        assert_eq!(note, AtomType::Note);

        let gotcha: AtomType = serde_json::from_str("\"gotcha\"").unwrap();
        assert_eq!(gotcha, AtomType::Gotcha);
    }

    #[test]
    fn test_confidence_display() {
        assert_eq!(Confidence::High.to_string(), "high");
        assert_eq!(Confidence::Medium.to_string(), "medium");
        assert_eq!(Confidence::Low.to_string(), "low");
    }

    #[test]
    fn test_confidence_serialization() {
        assert_eq!(
            serde_json::to_string(&Confidence::High).unwrap(),
            "\"high\""
        );
        assert_eq!(
            serde_json::to_string(&Confidence::Medium).unwrap(),
            "\"medium\""
        );
    }

    #[test]
    fn test_atom_new() {
        let atom = Atom::new(
            "K-000001".to_string(),
            "Test Title".to_string(),
            AtomType::Recipe,
            Confidence::High,
            "A brief summary".to_string(),
        );

        assert_eq!(atom.id, "K-000001");
        assert_eq!(atom.title, "Test Title");
        assert_eq!(atom.atom_type, AtomType::Recipe);
        assert_eq!(atom.confidence, Confidence::High);
        assert_eq!(atom.summary, "A brief summary");
        assert!(atom.details.is_none());
        assert!(atom.pitfalls.is_empty());
        assert!(atom.tags.is_empty());
        assert!(atom.sources.is_empty());
        assert!(atom.links.is_empty());
    }

    #[test]
    fn test_atom_yaml_serialization() {
        let atom = Atom::new(
            "K-000001".to_string(),
            "Error Handling".to_string(),
            AtomType::Recipe,
            Confidence::High,
            "Use Result for recoverable errors".to_string(),
        );

        let yaml = serde_yaml::to_string(&atom).unwrap();
        assert!(yaml.contains("id: K-000001"));
        assert!(yaml.contains("title: Error Handling"));
        assert!(yaml.contains("type: recipe"));
        assert!(yaml.contains("confidence: high"));
    }

    #[test]
    fn test_atom_yaml_deserialization() {
        let yaml = r#"
id: K-000042
title: Test Atom
type: gotcha
confidence: medium
summary: Watch out for this
tags:
  - rust
  - testing
updated_at: 2026-01-28
"#;
        let atom: Atom = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(atom.id, "K-000042");
        assert_eq!(atom.title, "Test Atom");
        assert_eq!(atom.atom_type, AtomType::Gotcha);
        assert_eq!(atom.confidence, Confidence::Medium);
        assert_eq!(atom.tags, vec!["rust", "testing"]);
    }

    #[test]
    fn test_atom_with_all_fields() {
        let yaml = r#"
id: K-000001
title: Full Atom
type: decision
confidence: high
summary: We chose X over Y
details: |
  Extended explanation here.
  Multiple lines supported.
pitfalls:
  - Don't forget to handle edge case A
  - Watch out for B
tags:
  - architecture
  - design
sources:
  - src/lib.rs
  - https://example.com/docs
links:
  - K-000002
  - other-project/K-000003
updated_at: 2026-01-28
"#;
        let atom: Atom = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(atom.atom_type, AtomType::Decision);
        assert!(atom.details.is_some());
        assert_eq!(atom.pitfalls.len(), 2);
        assert_eq!(atom.sources.len(), 2);
        assert_eq!(atom.links.len(), 2);
    }
}
