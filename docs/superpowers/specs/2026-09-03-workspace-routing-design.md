# P0-2 Workspace Routing Design

## Goal

Route native Codex Responses requests to different stored providers according
to the workspace recorded for the request's Codex session, while preserving the
existing fixed-provider and current-provider behavior.

## Scope

This change adds workspace route rules, request-time session/workspace lookup,
provider selection precedence, and CLI rule management. It does not add a web
UI, Chat Completions or Anthropic translation, OAuth, retries/failover, or
Codex live-file writes.

## Data Model

The local provider database gains a route-rules table with one row per
normalized workspace. Each row stores:

- normalized absolute workspace path;
- target local provider ID;
- creation and update timestamps.

Rules are unique by workspace. Adding a rule replaces the existing rule for
that workspace only when the command explicitly requests replacement; otherwise
the command reports a conflict. A rule can only reference an existing provider.
This slice does not expose provider deletion. If a rule points to a provider
row that is absent due to external database maintenance, route selection treats
it as unavailable and falls back to the current provider; the rule remains
visible in `route rule list` so it can be repaired or removed explicitly.

## Request Flow

For `/responses` and `/responses/compact`:

1. A fixed `--provider` selection, when configured, is used for every request.
2. Otherwise extract a Codex session ID from the request body/header using the
   stable fields already used by the project (`session_id`, `x-session-id`, and
   `metadata.session_id`). `previous_response_id` is not a session identity.
3. Build or refresh the read-only `SessionWorkspaceIndex` for the configured
   Codex home and resolve the session to a normalized workspace.
4. An exact workspace rule selects its provider.
5. If any lookup step has no usable result, select the stored current provider.

Unknown sessions, missing workspaces, and conflicting workspace metadata do not
guess a route. They fall back to the current provider. A request is rejected
only when the selected provider cannot be loaded or has invalid Responses
configuration, matching the existing route behavior.

The index is refreshed per request with bounded rollout scanning. Both the
normal Responses and compact endpoints use the same selection path and retain
upstream headers, credentials, status, body streaming, and SSE behavior.

## CLI

Add these commands under `route rule`:

```text
codex-route route rule add --workspace <path> --provider <id> [--replace]
codex-route route rule list
codex-route route rule remove --workspace <path>
```

The command uses the same `--data-dir` and `--codex-home` configuration sources
where relevant. `list` emits normalized paths and provider IDs without
credentials.

## Error Handling

Invalid or relative workspace paths are rejected by the CLI. Missing providers
are rejected when creating a rule. At request time, an unavailable rule target
falls back to the current provider; if no current provider exists, the existing
503 provider-unavailable response is returned. Provider configuration errors
continue to use the existing structured route error shape.

## Verification

Tests must cover:

- route rule insert/list/remove and duplicate behavior;
- workspace path normalization and provider validation;
- fixed-provider, workspace-rule, and current-provider precedence;
- session hit, unknown session, missing workspace, and conflicting workspace
  fallback behavior;
- two mock upstreams receiving requests from two mapped workspaces;
- existing Responses, compact, credential replacement, SSE, and error tests.

The acceptance command set is:

```text
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```
