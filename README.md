# codex-route

`codex-route` currently provides a local, read-only utility for listing Codex
sessions and resolving a Codex `session_id` to the workspace recorded in Codex
rollout metadata. This is the first milestone of the future workspace-aware LLM
gateway.

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

To list every unique session ID found in active and archived rollouts:

```bash
cargo run -- list --codex-home "$HOME/.codex"
```

The command emits a stable, lexically sorted JSON array:

```json
[
  "01a05cdb-cfc1-7853-a2f4-b6047652da9a",
  "01a05d11-1234-5678-9abc-def012345678"
]
```

Codex home resolution is:

```text
--codex-home -> CODEX_HOME -> $HOME/.codex
```

The default rollout scan limit is 64 KiB per file and can be changed with
`--max-rollout-bytes` up to 4 MiB.

## Codex Providers

`codex-route` keeps its own provider database at
`<user config directory>/codex-route/codex-route.db` by default. Use
`--data-dir` to select another directory. Provider records retain the raw
Codex `settingsConfig` and cc-switch metadata so a later routing layer can use
fields that are not yet normalized by this CLI.

List stored providers without printing credentials:

```bash
cargo run -- provider list --data-dir /path/to/data
```

Show one provider. Credential fields are replaced with `[REDACTED]` unless
`--reveal-secrets` is explicitly supplied:

```bash
cargo run -- provider show custom --data-dir /path/to/data
cargo run -- provider show custom --data-dir /path/to/data --reveal-secrets
```

Import only rows whose cc-switch `app_type` is `codex`:

```bash
cargo run -- provider import-cc-switch \
  --data-dir /path/to/data \
  --cc-switch-db "$HOME/.cc-switch/cc-switch.db"
```

The source database is opened read-only. Malformed or proxy-placeholder rows
are reported in the JSON result and do not prevent valid rows from importing.
Repeated imports match the source provider ID. `--on-conflict` accepts
`skip` (default), `replace`, or `rename`; local-origin rows are never silently
overwritten. This milestone does not modify Codex `config.toml`/`auth.json`
and does not modify Codex live files.

## Local Responses Route

Start the loopback route using the current stored provider and optional
workspace route rules:

```bash
cargo run -- route serve \
  --data-dir /path/to/data \
  --codex-home "$HOME/.codex"
```

The route listens on `http://127.0.0.1:16729` and accepts Codex
`GET /v1/models`, `POST /v1/responses`, and
`POST /v1/responses/compact` requests. The same endpoints are also available
without the `/v1` prefix for Codex configurations whose `base_url` points at
the route root. Point Codex's active provider `base_url` at
`http://127.0.0.1:16729/v1`. Select a specific stored provider with
`--provider <id>` or change the current provider in the local store.

The route injects the credential from the local provider database and binds to
loopback only. `/v1/models` returns an empty Codex-compatible model catalog
until model-catalog projection is implemented. `/v1/responses/compact` is a
transparent upstream passthrough. When `--provider` is omitted, the route
selects a provider using the request's Codex session and workspace route rules.
The route scans a bounded rollout prefix per request so sessions created after
startup can be resolved without restarting the server. This milestone forwards
only the native Responses protocol; it does not translate Chat Completions or
Anthropic requests, write Codex live files, or perform retries/failover.

Manage workspace route rules from the CLI:

```bash
cargo run -- route rule add \
  --data-dir /path/to/data \
  --workspace /path/to/project \
  --provider provider-a

cargo run -- route rule list --data-dir /path/to/data
cargo run -- route rule remove \
  --data-dir /path/to/data \
  --workspace /path/to/project
```

Adding an existing workspace requires `--replace`. Rules store normalized
absolute paths and can reference only providers already in the local store.

## Selection Rules

- Multiple threads with the same `session_id` are treated as one project.
- If all matching records have the same normalized workspace, that path is returned.
- If records contain multiple workspaces, the oldest valid `session_meta` workspace is selected as `workspace` and all candidates are listed in `workspaces`.
- Existing paths are canonicalized; missing paths are returned as normalized absolute paths with `workspace_exists: false`.
- Malformed rollout lines are ignored. Prompt and conversation records are never read after the first usable `session_meta`.
- Route provider precedence is `--provider` > exact workspace rule > current provider.
- Requests without a usable session, unknown sessions, conflicting workspace metadata, missing workspaces, or unreadable rollout indexes fall back to the current provider.

## Scope Boundary

This milestone does not read `state_5.sqlite`, run a background session watcher,
parse `X-Codex-Turn-Metadata`, proxy WebSocket traffic, or infer parent
workspaces from `forked_from_thread_id`. Each routed request builds a fresh
bounded read-only snapshot. The lookup library is separated from the CLI so
the route can reuse the provider store directly. There is no UI in this MVP
slice.

Exit codes are stable for automation:

```text
0  query succeeded
2  invalid arguments or empty session ID
3  session ID not found
4  Codex home, rollout, scan, or output failure
```
