# Atlas MCP

Centralized knowledge management server implementing the Model Context Protocol (MCP). Provides persistent long-term memory for AI agents organized by `org/project/atom` hierarchy.

## Features

- **Automatic context detection** - Extracts org/project from git remote URL
- **Hierarchical organization** - Knowledge stored as `~/.atlas/orgs/{org}/{project}/atoms/`
- **Version-controlled storage** - Store atoms in your repo with `init --create_symlink`
- **Cross-project links** - Reference atoms across projects within the same org
- **Simple atom model** - 4 types: note, gotcha, recipe, decision
- **Dual transport** - STDIO (default) or HTTP/SSE

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
```

## Usage

### STDIO (default)

```bash
atlas-mcp
```

### Custom storage path

```bash
atlas-mcp --storage /path/to/knowledge
```

### HTTP/SSE

```bash
atlas-mcp --http 3000
```

## MCP Configuration

### Claude Code

Add to `~/.claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "atlas": {
      "command": "/path/to/atlas-mcp"
    }
  }
}
```

### With custom storage path

```json
{
  "mcpServers": {
    "atlas": {
      "command": "/path/to/atlas-mcp",
      "args": ["--storage", "/path/to/knowledge"]
    }
  }
}
```

### With custom working directory

```json
{
  "mcpServers": {
    "atlas": {
      "command": "/path/to/atlas-mcp",
      "env": {
        "ATLAS_CWD": "/path/to/project"
      }
    }
  }
}
```

## Tools

| Tool | Description |
|------|-------------|
| `search` | Search atoms by query, type, tags, confidence |
| `upsert` | Create or update an atom |
| `get_atom` | Get full atom content by ID |
| `list_atoms` | List atoms with optional filtering |
| `delete_atom` | Delete an atom |
| `init` | Initialize a new org/project |
| `list_projects` | List all projects across organizations |
| `get_context` | Get detected org/project context |

## Atom Model

```yaml
id: K-000001
title: "Error handling pattern"
type: recipe               # note, gotcha, recipe, decision
confidence: high           # high, medium, low
summary: "Brief explanation"
details: "Extended content"  # optional
pitfalls: ["Watch out..."]   # optional
tags: [rust, error-handling]
sources: [src/lib.rs]        # paths or URLs
links: [K-000042, api/K-000010]  # id or project/id
updated_at: 2026-01-28
```

### Types

| Type | Purpose |
|------|---------|
| `note` | Facts, observations, general knowledge |
| `gotcha` | Warnings, pitfalls, things to avoid |
| `recipe` | How-to guides, patterns, code snippets |
| `decision` | Architectural rationale, why X over Y |

## Context Detection

Atlas automatically detects the current project context:

1. **Git remote** - Parses `git remote get-url origin` for org/project
2. **Fallback** - Uses `global/{directory_name}`

### Supported git URL formats

- `git@github.com:org/project.git`
- `https://github.com/org/project.git`
- `git@gitlab.com:org/project.git`

## Storage Structure

### Default (central storage)

```
~/.atlas/
├── orgs/
│   └── {org}/
│       └── {project}/
│           ├── index.yaml
│           └── atoms/
│               └── K-XXXXXX.yaml
```

### Version-controlled (repo storage)

Use `init` with `create_symlink: true` to store atoms in your repo:

```
{repo}/.atlas/
├── index.yaml
└── atoms/
    └── K-XXXXXX.yaml

~/.atlas/orgs/{org}/{project} → {repo}/.atlas/  (symlink)
```

This allows:
- Atoms to be version-controlled via git
- Knowledge to be shared with collaborators
- `git pull` to update local knowledge

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ATLAS_CWD` | Override working directory for context detection |
| `ATLAS_STORAGE` | Override storage root (default: `~/.atlas`) |

## Development

```bash
# Run tests
cargo test

# Build release
cargo build --release

# Run with debug output
RUST_LOG=debug cargo run
```

## License

MIT
