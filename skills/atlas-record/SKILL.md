---
name: atlas-record
description: Record new knowledge to Atlas long-term memory. Use after completing non-trivial work (bug fixes, features, refactors), discovering patterns or gotchas, making architectural decisions, or learning something reusable. Triggers on: "record this", "save to Atlas", "remember this pattern", "add a gotcha", or when suggesting knowledge worth preserving.
---

# Atlas Record

Capture reusable learnings in Atlas.

## Context Check

Before recording, verify Atlas context:

1. **Run `atlas context`** to see current org/project
2. **If context is unknown** (`source: fallback` or missing org/project):
   - **Run `atlas projects`** to list known projects
   - **If a project name matches** the repo you are working on, **use the org suggested by the list** with `atlas activate_project`
   - **If no matching project exists**, it is OK to **infer org from the file path** and then `atlas activate_project`
3. **Check the `source` field**:
   - `git_remote` or `activated` = Good to proceed

Recording to the wrong project makes knowledge unfindable.

## Atom Types

| Type | Use for |
|------|---------|
| `note` | Verified facts, conventions, domain knowledge |
| `gotcha` | Warnings, pitfalls, "watch out for X" |
| `recipe` | Patterns, how-to guides, code snippets |
| `decision` | Architectural choices, why X over Y |

## Workflow

1. **Search first** - Avoid duplicates with `atlas search <query>`
2. **Evaluate** - Is this reusable, non-obvious, stable, actionable?
3. **Create atom** with `atlas upsert`:
   - `--title`: Clear, searchable name
   - `--type`: gotcha, recipe, decision, or note
   - `--summary`: The knowledge (markdown supported)
   - `--confidence`: high, medium, or low
   - `--tag`: Keywords for searchability (repeatable)
   - `--source`: Relevant file paths (repeatable)
4. **Link related atoms** with `atlas link <SOURCE> <TARGET>` if applicable

## CLI Commands

**Search for duplicates:**
```bash
atlas search "error handling"
```

**Create new atom:**
```bash
atlas upsert \
  --title "API rate limits require exponential backoff" \
  --type gotcha \
  --confidence high \
  --summary "The external API enforces strict rate limits..." \
  --tag api --tag rate-limiting \
  --source src/api/client.rs
```

**Link related atoms:**
```bash
atlas link K-000012 K-000045
```

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
