# Atlas MCP

Centralized knowledge management server implementing the Model Context Protocol (MCP). Provides persistent long-term memory for AI agents organized by `org/project/atom` hierarchy.

## Features

- **Automatic context detection** - Extracts org/project from git remote URL
- **Hierarchical organization** - Knowledge stored as `~/.atlas/orgs/{org}/{project}/atoms/`
- **Version-controlled storage** - Store atoms in your repo with `enable_local_storage`
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

### With explicit org/project

When the MCP client can't detect git context (e.g., sandboxed environments):

```bash
atlas-mcp --org my-company --project my-service
```

### With project root for local storage

Specify where `.atlas/` should be created when using `enable_local_storage`:

```bash
atlas-mcp --org my-company --project my-service --project-root /path/to/repo
```

### Custom storage path

```bash
atlas-mcp --storage /path/to/knowledge
```

### HTTP/SSE

```bash
atlas-mcp --http 3000
```

In HTTP mode, clients set context via headers on each request:
- `X-Atlas-Org: my-company`
- `X-Atlas-Project: my-service`

Note: The `enable_local_storage` tool is not available in HTTP mode (local filesystem access required).

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

### With explicit org/project (recommended for sandboxed clients)

```json
{
  "mcpServers": {
    "atlas": {
      "command": "/path/to/atlas-mcp",
      "args": ["--org", "my-company", "--project", "my-service"]
    }
  }
}
```

### With project root for local storage

```json
{
  "mcpServers": {
    "atlas": {
      "command": "/path/to/atlas-mcp",
      "args": [
        "--org", "my-company",
        "--project", "my-service",
        "--project-root", "/path/to/repo"
      ]
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

### With custom working directory (legacy)

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
| `enable_local_storage` | Enable version-controlled storage in project root (stdio only) |
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

Atlas detects the current project context using the following priority:

| Priority | Source | Description |
|----------|--------|-------------|
| 1 | HTTP headers | `X-Atlas-Org` and `X-Atlas-Project` (HTTP mode only) |
| 2 | CLI args | `--org` and `--project` flags |
| 3 | Git remote | Parses `git remote get-url origin` |
| 4 | Fallback | Uses `global/{directory_name}` |

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

### Version-controlled (local storage)

Use `enable_local_storage` to store atoms in your repo:

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
| `ATLAS_ORG` | Override organization (set via `--org` CLI arg) |
| `ATLAS_PROJECT` | Override project (set via `--project` CLI arg) |
| `ATLAS_PROJECT_ROOT` | Project root for local storage (set via `--project-root` CLI arg) |
| `ATLAS_CWD` | Override working directory for git context detection |
| `ATLAS_STORAGE` | Override storage root (default: `~/.atlas`) |

## Development

```bash
# Set up git hooks (auto-formats code on commit)
git config core.hooksPath .githooks

# Run tests
cargo test

# Build release
cargo build --release

# Run with debug output
RUST_LOG=debug cargo run
```

## License

MIT
