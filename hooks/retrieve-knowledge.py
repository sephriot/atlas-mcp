#!/usr/bin/env python3
"""
Retrieve Atlas knowledge relevant to the user's prompt.
Searches for relevant atoms, then retrieves full content for each.
"""
import json
import subprocess
import sys


def run_atlas(args: list[str]) -> str | None:
    """Run atlas CLI command and return output."""
    try:
        result = subprocess.run(
            ["atlas"] + args,
            capture_output=True,
            text=True,
            timeout=10
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass
    return None


def main():
    # Read hook input from stdin
    try:
        hook_input = json.load(sys.stdin)
    except json.JSONDecodeError:
        sys.exit(0)

    prompt = hook_input.get("prompt", "")
    if not prompt:
        sys.exit(0)

    # Search Atlas using the prompt
    search_output = run_atlas(["search", prompt, "--page-size", "5", "--json"])
    if not search_output:
        sys.exit(0)

    try:
        response = json.loads(search_output)
        # Handle new paginated response format
        results = response.get("results", []) if isinstance(response, dict) else response
    except json.JSONDecodeError:
        sys.exit(0)

    if not results:
        sys.exit(0)

    # Retrieve full atom content for each search result
    atoms = []
    for result in results:
        atom_id = result.get("id")
        if atom_id:
            atom_content = run_atlas(["get", atom_id])
            if atom_content:
                atoms.append(f"[{atom_id}]\n{atom_content}")

    if not atoms:
        sys.exit(0)

    # Build context with full atom content
    context = "**Atlas LTM - Relevant knowledge:**\n\n"
    context += "\n---\n".join(atoms)

    # Return as additionalContext
    output = {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": context
        }
    }
    print(json.dumps(output))


if __name__ == "__main__":
    main()
