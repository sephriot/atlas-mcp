---
name: atlas-record
description: Record new knowledge to Atlas long-term memory. Use after completing non-trivial work (bug fixes, features, refactors), discovering patterns or gotchas, making architectural decisions, or learning something reusable. Triggers on: "record this", "save to Atlas", "remember this pattern", "add a gotcha", or when suggesting knowledge worth preserving.
---

# Atlas Record

Capture reusable learnings in Atlas.

## Atom Types

| Type | Use for |
|------|---------|
| `note` | Verified facts, conventions, domain knowledge |
| `gotcha` | Warnings, pitfalls, "watch out for X" |
| `recipe` | Patterns, how-to guides, code snippets |
| `decision` | Architectural choices, why X over Y |

## Workflow

1. **Search first** - Avoid duplicates with `mcp__atlas-mcp__search`
2. **Evaluate** - Is this reusable, non-obvious, stable, actionable?
3. **Create atom** with `mcp__atlas-mcp__upsert`:
   - `title`: Clear, searchable name
   - `type`: gotcha, recipe, decision, or note
   - `summary`: The knowledge (markdown supported)
   - `confidence`: high, medium, or low
   - `tags`: Keywords for searchability (JSON array)
   - `sources`: Relevant file paths (JSON array)
4. **Link related atoms** with `mcp__atlas-mcp__link` if applicable

## Quality Criteria

Only record if knowledge is:
- **Reusable** - Applies beyond this specific instance
- **Non-obvious** - Not easily discoverable from code/docs
- **Stable** - Unlikely to change frequently
- **Actionable** - Helps make decisions or avoid mistakes

## Anti-Patterns

- Creating atoms for trivial/obvious information
- Duplicating existing knowledge
- Recording temporary workarounds as permanent
- Atoms too specific to be reusable

Creating nothing is better than creating low-value atoms.
