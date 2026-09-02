# codex-route

`codex-route` currently provides a local, read-only utility for resolving a
Codex `session_id` to the workspace recorded in Codex rollout metadata. This is
the first milestone of the future workspace-aware LLM gateway.

## Current MVP

The `resolve` command scans these directories under the Codex home:

```text
sessions/
archived_sessions/
```

For each `rollout-*.jsonl` or `rollout-*.jsonl.zst` file, it reads only the
bounded prefix needed to find the first `session_meta` record. It extracts the
record's `session_id`, thread `id`, and absolute `cwd`, then groups all matching
thread records under the same session ID.

Run it with:

```bash
cargo run -- resolve \
  --codex-home "$HOME/.codex" \
  --session-id 01a05cdb-cfc1-7853-a2f4-b6047652da9a
```

The command emits JSON like:

```json
{
  "session_id": "session-1",
  "workspace": "/Users/me/project-a",
  "workspace_exists": true,
  "workspaces": ["/Users/me/project-a"],
  "thread_ids": ["thread-1", "thread-2"],
  "rollout_paths": [
    "/Users/me/.codex/sessions/2026/09/02/rollout-2026-09-02T12-00-00-session-1.jsonl"
  ],
  "conflicting_workspaces": false
}
```

Codex home resolution is:

```text
--codex-home -> CODEX_HOME -> $HOME/.codex
```

The default rollout scan limit is 64 KiB per file and can be changed with
`--max-rollout-bytes` up to 4 MiB.

## Selection Rules

- Multiple threads with the same `session_id` are treated as one project.
- If all matching records have the same normalized workspace, that path is returned.
- If records contain multiple workspaces, the oldest valid `session_meta` workspace is selected as `workspace` and all candidates are listed in `workspaces`.
- Existing paths are canonicalized; missing paths are returned as normalized absolute paths with `workspace_exists: false`.
- Malformed rollout lines are ignored. Prompt and conversation records are never read after the first usable `session_meta`.

## Scope Boundary

This milestone does not read `state_5.sqlite`, watch for session changes, parse
`X-Codex-Turn-Metadata`, select providers, proxy HTTP/SSE or WebSocket traffic,
or infer parent workspaces from `forked_from_thread_id`. Each command invocation
builds a fresh read-only snapshot. The lookup library is separated from the
CLI so a later gateway can reuse it directly.

Exit codes are stable for automation:

```text
0  session resolved
2  invalid arguments or empty session ID
3  session ID not found
4  Codex home, rollout, scan, or output failure
```
