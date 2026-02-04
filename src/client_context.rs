use clap::ValueEnum;
use std::fmt;

/// Client context determines which instructions are shown to the AI agent.
/// Different clients have different workflows and emphases.
#[derive(Debug, Clone, Copy, Default, ValueEnum, PartialEq, Eq)]
pub enum ClientContext {
    /// Claude Code - agentic coding with proactive knowledge use
    #[default]
    ClaudeCode,
    /// IDE assistants (VSCode, Cursor) - reactive, code-focused
    Ide,
    /// OpenAI Codex CLI - code generation focused
    Codex,
}

impl ClientContext {
    /// Returns the instructions tailored for this client context.
    pub fn instructions(&self) -> &'static str {
        match self {
            ClientContext::ClaudeCode => INSTRUCTIONS_CLAUDE_CODE,
            ClientContext::Ide => INSTRUCTIONS_IDE,
            ClientContext::Codex => INSTRUCTIONS_CODEX,
        }
    }
}

impl fmt::Display for ClientContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientContext::ClaudeCode => write!(f, "claude-code"),
            ClientContext::Ide => write!(f, "ide"),
            ClientContext::Codex => write!(f, "codex"),
        }
    }
}

impl std::str::FromStr for ClientContext {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude-code" => Ok(ClientContext::ClaudeCode),
            "ide" => Ok(ClientContext::Ide),
            "codex" => Ok(ClientContext::Codex),
            _ => Err(format!(
                "unknown client context '{}', expected: claude-code, ide, codex",
                s
            )),
        }
    }
}

const INSTRUCTIONS_CLAUDE_CODE: &str = r#"Atlas MCP - Long-term memory for AI agents.

WORKFLOW (optimized for Claude Code):
1. SEARCH first - Before any task, search Atlas for patterns/gotchas/decisions
2. READ full atoms - Use get for each relevant search result
3. APPLY knowledge - Let retrieved atoms constrain your approach
4. RECORD learnings - After completing work, upsert discoveries as atoms
5. LINK related atoms - Connect related knowledge for future retrieval

Use Atlas proactively: search before planning, record after learning.

ATOM TYPES:
- note: Facts, observations, domain knowledge
- gotcha: Warnings, pitfalls, "watch out for X"
- recipe: Patterns, how-to guides, code snippets
- decision: Architectural rationale, why X over Y

ATOM IDs:
- All IDs returned as full paths: "org/project/K-000001"
- Accepts: full path, "project/K-000001", or bare "K-000001" (context fills gaps)
- Cross-project/org operations use full paths

LINKING:
- Use link/unlink to create directed connections between atoms
- Links are directed: A links to B does NOT mean B links to A
- Cross-project links supported within same org

CONTEXT: Auto-detected from git remote. Use context tool to verify.

CITATION: Reference atom IDs in reasoning, e.g., [K-000042]

PARAMETER FORMAT:
- Array fields (tags, sources, links, pitfalls) MUST be JSON arrays, NOT strings
- CORRECT: tags: ["api", "rust"]
- WRONG: tags: "[\"api\", \"rust\"]""#;

const INSTRUCTIONS_IDE: &str = r#"Atlas MCP - Long-term memory for AI agents.

WORKFLOW (optimized for IDE assistants):
1. SEARCH - Query Atlas when user asks about patterns or conventions
2. GET - Retrieve full atom content for detailed guidance
3. APPLY - Use retrieved knowledge to inform code suggestions
4. RECORD - Capture useful patterns discovered during coding sessions

Focus on code patterns, conventions, and project-specific knowledge.

ATOM TYPES:
- note: Facts, observations, domain knowledge
- gotcha: Warnings, pitfalls, "watch out for X"
- recipe: Patterns, how-to guides, code snippets
- decision: Architectural rationale, why X over Y

ATOM IDs: Accepts full path, project/id, or bare id.

CONTEXT: Auto-detected from git remote.

PARAMETER FORMAT:
- Array fields (tags, sources, links, pitfalls) MUST be JSON arrays, NOT strings
- CORRECT: tags: ["api", "rust"]
- WRONG: tags: "[\"api\", \"rust\"]""#;

const INSTRUCTIONS_CODEX: &str = r#"Atlas MCP - Long-term memory for AI agents.

WORKFLOW (optimized for Codex CLI):
1. SEARCH - Query for relevant context before generating code
2. GET - Retrieve specific atoms for detailed patterns
3. UPSERT - Record reusable patterns after successful generations
4. LINK - Connect related knowledge atoms

Prioritize recipes and gotchas for code generation tasks.

ATOM TYPES: note, gotcha, recipe, decision

ATOM IDs: Full path (org/project/id), project/id, or bare id

CONTEXT: Auto-detected from git remote

PARAMETER FORMAT:
- Array fields (tags, sources, links, pitfalls) MUST be JSON arrays, NOT strings
- CORRECT: tags: ["api", "rust"]
- WRONG: tags: "[\"api\", \"rust\"]""#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_claude_code() {
        assert_eq!(ClientContext::default(), ClientContext::ClaudeCode);
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            "claude-code".parse::<ClientContext>().unwrap(),
            ClientContext::ClaudeCode
        );
        assert_eq!("ide".parse::<ClientContext>().unwrap(), ClientContext::Ide);
        assert_eq!(
            "codex".parse::<ClientContext>().unwrap(),
            ClientContext::Codex
        );
        assert_eq!("IDE".parse::<ClientContext>().unwrap(), ClientContext::Ide);
        assert!("unknown".parse::<ClientContext>().is_err());
    }

    #[test]
    fn test_display() {
        assert_eq!(ClientContext::ClaudeCode.to_string(), "claude-code");
        assert_eq!(ClientContext::Ide.to_string(), "ide");
        assert_eq!(ClientContext::Codex.to_string(), "codex");
    }

    #[test]
    fn test_instructions_not_empty() {
        assert!(!ClientContext::ClaudeCode.instructions().is_empty());
        assert!(!ClientContext::Ide.instructions().is_empty());
        assert!(!ClientContext::Codex.instructions().is_empty());
    }
}
