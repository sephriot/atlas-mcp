use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AtomType, Confidence};

/// A lightweight index entry for fast searching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexEntry {
    /// Atom ID
    pub id: String,

    /// Short descriptive title
    pub title: String,

    /// Type of knowledge
    #[serde(rename = "type")]
    pub atom_type: AtomType,

    /// Confidence level
    pub confidence: Confidence,

    /// Keywords for search
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Source references (file paths or URLs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,

    /// Related atoms
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
}

impl IndexEntry {
    pub fn from_atom(atom: &super::Atom) -> Self {
        Self {
            id: atom.id.clone(),
            title: atom.title.clone(),
            atom_type: atom.atom_type,
            confidence: atom.confidence,
            tags: atom.tags.clone(),
            sources: atom.sources.clone(),
            links: atom.links.clone(),
        }
    }
}

/// The project index containing all atom entries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Index {
    /// Next ID counter
    pub next_id: u32,

    /// All atom entries
    #[serde(default)]
    pub entries: Vec<IndexEntry>,
}

impl Index {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            entries: Vec::new(),
        }
    }

    /// Generate the next atom ID and increment counter.
    pub fn generate_id(&mut self) -> String {
        let id = format!("K-{:06}", self.next_id);
        self.next_id += 1;
        id
    }

    /// Add or update an entry in the index.
    pub fn upsert_entry(&mut self, entry: IndexEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.id == entry.id) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Remove an entry from the index by ID.
    pub fn remove_entry(&mut self, id: &str) -> Option<IndexEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, title: &str) -> IndexEntry {
        IndexEntry {
            id: id.to_string(),
            title: title.to_string(),
            atom_type: AtomType::Note,
            confidence: Confidence::High,
            tags: vec![],
            sources: vec![],
            links: vec![],
        }
    }

    #[test]
    fn test_index_new() {
        let index = Index::new();
        assert_eq!(index.next_id, 1);
        assert!(index.entries.is_empty());
    }

    #[test]
    fn test_generate_id() {
        let mut index = Index::new();
        assert_eq!(index.generate_id(), "K-000001");
        assert_eq!(index.generate_id(), "K-000002");
        assert_eq!(index.generate_id(), "K-000003");
        assert_eq!(index.next_id, 4);
    }

    #[test]
    fn test_generate_id_formatting() {
        let mut index = Index {
            next_id: 999999,
            entries: vec![],
        };
        assert_eq!(index.generate_id(), "K-999999");
        assert_eq!(index.generate_id(), "K-1000000");
    }

    #[test]
    fn test_upsert_entry_insert() {
        let mut index = Index::new();
        let entry = make_entry("K-000001", "Test Entry");

        index.upsert_entry(entry);

        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].id, "K-000001");
        assert_eq!(index.entries[0].title, "Test Entry");
    }

    #[test]
    fn test_upsert_entry_update() {
        let mut index = Index::new();
        index.upsert_entry(make_entry("K-000001", "Original"));
        index.upsert_entry(make_entry("K-000001", "Updated"));

        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].title, "Updated");
    }

    #[test]
    fn test_upsert_multiple_entries() {
        let mut index = Index::new();
        index.upsert_entry(make_entry("K-000001", "First"));
        index.upsert_entry(make_entry("K-000002", "Second"));
        index.upsert_entry(make_entry("K-000003", "Third"));

        assert_eq!(index.entries.len(), 3);
    }

    #[test]
    fn test_remove_entry_exists() {
        let mut index = Index::new();
        index.upsert_entry(make_entry("K-000001", "Test"));

        let removed = index.remove_entry("K-000001");

        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "K-000001");
        assert!(index.entries.is_empty());
    }

    #[test]
    fn test_remove_entry_not_exists() {
        let mut index = Index::new();
        index.upsert_entry(make_entry("K-000001", "Test"));

        let removed = index.remove_entry("K-999999");

        assert!(removed.is_none());
        assert_eq!(index.entries.len(), 1);
    }

    #[test]
    fn test_index_serialization() {
        let mut index = Index::new();
        index.upsert_entry(make_entry("K-000001", "Test"));

        let yaml = serde_yaml::to_string(&index).unwrap();
        let parsed: Index = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.next_id, index.next_id);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].id, "K-000001");
    }
}
