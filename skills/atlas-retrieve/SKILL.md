---
name: atlas-retrieve
description: Retrieve knowledge from Atlas long-term memory. Use when starting any task, answering questions about the codebase, or needing context about patterns, gotchas, decisions, or conventions. Triggers on: "what do we know about X", "check Atlas for", "search knowledge", "find patterns", "any gotchas", or at the start of any non-trivial task.
---

# Atlas Retrieve

Retrieve relevant knowledge from Atlas before taking action.

## Workflow

1. **Extract keywords** from the task/question
2. **Search Atlas** with those keywords using `atlas search <query>`
3. **Read full atoms** with `atlas get <ID>` for each relevant hit
4. **Cite atoms** in your response using `[K-XXXXXX]` format

## CLI Commands

**Broad discovery:**
```bash
atlas search "error handling" --page-size 10
```

**Type-specific:**
```bash
atlas search --type gotcha --tag api
```

**High-confidence only:**
```bash
atlas search "authentication" --confidence high
```

**Get full atom content:**
```bash
atlas get K-000012
```

## Output Format

Always report what was found:

```
**Atlas Context:**
- [K-000012] Error Handling: Applied retry pattern from this recipe
- [K-000045] API Gotcha: Avoided rate limit issue per this warning

**Knowledge Gaps:**
- No existing pattern for X
```

If nothing relevant found, state: "No relevant Atlas knowledge found for [keywords]."
