# P0-2 Workspace Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route native Codex Responses requests to a provider selected by the
request session's recorded workspace, with durable CLI-managed workspace rules.

**Architecture:** Keep provider and rule persistence inside `ProviderStore`,
put absolute-path normalization in a small workspace-rule domain module, and
make `RouteState` own the Codex scan configuration. Dynamic requests buffer the
JSON body once, resolve `session_id`/`x-session-id`/`metadata.session_id`, scan
the bounded rollout index, and select a provider before forwarding. Fixed
provider selection remains an explicit override.

**Tech Stack:** Rust 2021, Axum 0.7, Tokio, Reqwest, rusqlite, serde/serde_json,
TOML, Clap, existing rollout/index modules.

**Spec:** `docs/superpowers/specs/2026-09-03-workspace-routing-design.md`

## Global Constraints

- Preserve native Responses and `/responses/compact` passthrough, provider credential replacement, SSE response streaming, and existing error shapes.
- Provider selection precedence is `--provider` > exact normalized workspace rule > current provider.
- Unknown sessions, missing workspaces, conflicting workspace metadata, malformed request JSON, and scan failures fall back to the current provider.
- `previous_response_id` is never used as a session identity.
- Rules store normalized absolute paths and may reference only an existing provider when created; this slice does not expose provider deletion.
- Do not add UI, Chat/Anthropic conversion, OAuth, retries/failover, or Codex live-file writes.
- Run focused tests after each task and the complete `cargo test --locked`, fmt check, and clippy check before finishing.

---

### Task 1: Add Workspace Rule Domain and Store Migration

**Files:**
- Create: `src/workspace_rule.rs`
- Modify: `src/lib.rs`
- Modify: `src/provider_store.rs:10-280`
- Test: `tests/provider_store.rs`

**Interfaces:**
- Produces `WorkspaceRouteRule { workspace: PathBuf, provider_id: String, created_at: i64, updated_at: i64 }`.
- Produces `normalize_workspace_path(&Path) -> Result<PathBuf, WorkspacePathError>`.
- Produces `ProviderStore::list_route_rules`, `upsert_route_rule`, and `remove_route_rule`.

- [ ] **Step 1: Write failing domain and persistence tests**

Add tests for absolute-path normalization, rejection of relative paths, route
rule insertion/listing, duplicate rejection without `replace`, replacement
with `replace`, removal, and rejection of a missing provider.

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `cargo test --test provider_store route_rule`

Expected: compile failures for the missing rule type and store methods.

- [ ] **Step 3: Implement the workspace-rule domain module**

Define the serializable rule record and normalize existing paths with
`fs::canonicalize`; normalize missing absolute paths lexically by removing `.`
and resolving `..`. Return an explicit error for empty or relative paths.

- [ ] **Step 4: Migrate the provider database to schema version 2**

Create `workspace_route_rules(workspace PRIMARY KEY, provider_id NOT NULL,
created_at NOT NULL, updated_at NOT NULL)` and an index on `provider_id`.
Existing version-1 databases must open and migrate via `PRAGMA user_version`.

- [ ] **Step 5: Implement transactional rule CRUD**

Normalize paths before SQL access, verify the provider exists inside the same
transaction as insertion/replacement, reject duplicate workspaces unless
`replace` is true, and return the normalized rule rows ordered by workspace.
Removing a missing rule returns a distinct not-found error.

- [ ] **Step 6: Run the focused tests and commit**

Run: `cargo test --test provider_store`

Expected: all provider-store tests pass.

```bash
git add src/workspace_rule.rs src/lib.rs src/provider_store.rs tests/provider_store.rs
git commit -m "feat: persist workspace route rules"
```

### Task 2: Add Request-Time Session and Workspace Provider Selection

**Files:**
- Modify: `src/index.rs:139-154`
- Modify: `src/route.rs:20-270`
- Test: `tests/route.rs`

**Interfaces:**
- Consumes `normalize_workspace_path` and the new `ProviderStore` rule APIs.
- Produces `RouteState::with_scan_config(store, provider_id, scan_config)`.
- Produces `extract_codex_session_id(&HeaderMap, &Value) -> Option<String>`.

- [ ] **Step 1: Add failing routing integration tests**

Create two rollout fixtures and two mock upstream providers. Assert that a
request with `session_id` reaches provider A, a request with
`metadata.session_id` reaches provider B, and an unknown session reaches the
current provider. Add assertions that conflicting workspace metadata and
missing workspace paths also use the current provider, while an explicit
`provider_id` always wins.

- [ ] **Step 2: Run the focused route tests and verify they fail**

Run: `cargo test --test route workspace_rule`

Expected: compile failure for the new scan-config constructor, or assertion
failure because every request still uses the current provider.

- [ ] **Step 3: Add request session extraction**

Check `session_id` then `x-session-id` headers, followed by
`body.metadata.session_id`. Trim and ignore empty values. Do not inspect
`previous_response_id`; malformed JSON is treated as “no session” for routing.

- [ ] **Step 4: Add request-time bounded index resolution**

Store optional Codex home and scan limit in `RouteState`. For dynamic requests,
build `SessionWorkspaceIndex` once per request, resolve the extracted session,
and use a rule only when the lookup has one non-conflicting, existing
workspace. Catch index/read/resolve failures and continue with current-provider
selection.

- [ ] **Step 5: Refactor forwarding to buffer only the request body**

Collect the Axum body once, parse it for routing, and pass the unchanged bytes
to Reqwest. Preserve all existing request headers except hop-by-hop and client
authorization headers; keep upstream response bytes and headers streaming as
before.

- [ ] **Step 6: Implement provider precedence and rule fallback**

Resolve the fixed provider first. Otherwise load the rule target by provider ID
only if it exists and has usable configuration; if the rule is stale or invalid,
select the current provider. Keep existing 503 responses for unavailable or
invalid selected providers.

- [ ] **Step 7: Run route tests and commit**

Run: `cargo test --test route`

Expected: all existing P0-1 tests and new workspace routing tests pass.

```bash
git add src/index.rs src/route.rs tests/route.rs
git commit -m "feat: route Codex requests by workspace"
```

### Task 3: Expose Rule Management and Scan Configuration in the CLI

**Files:**
- Modify: `src/main.rs:35-220`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes `ProviderStore` rule CRUD and `ScanConfig`.
- Produces `route rule add --workspace <path> --provider <id> [--replace]`.
- Produces `route rule list` and `route rule remove --workspace <path>`.
- Extends `route serve` with `--codex-home` and `--max-rollout-bytes`.

- [ ] **Step 1: Add failing CLI tests**

Create a provider-store fixture, invoke add/list/remove through the binary, and
assert JSON output contains the normalized workspace and provider ID. Assert a
duplicate add exits nonzero unless `--replace` is supplied, and `route --help`
shows the new command.

- [ ] **Step 2: Run the focused CLI tests and verify they fail**

Run: `cargo test --test cli route_rule`

Expected: Clap rejects the new subcommands before implementation.

- [ ] **Step 3: Add nested Clap route-rule commands**

Add `RouteCommand::Rule` with Add/List/Remove argument structs. Reuse
`--data-dir`; pass workspace paths through store normalization and emit
structured JSON results without provider settings or credentials.

- [ ] **Step 4: Wire route serve scan flags**

Flatten `ScanArgs` into `RouteServeArgs`, build `ScanConfig::from_cli`, and
construct `RouteState::with_scan_config` before startup validation.

- [ ] **Step 5: Map rule errors to stable CLI output and run tests**

Map duplicate/provider-not-found/not-found errors to existing nonzero CLI error
handling without changing resolve/list exit codes. Run `cargo test --test cli`.

- [ ] **Step 6: Commit the CLI deliverable**

```bash
git add src/main.rs tests/cli.rs
git commit -m "feat: add workspace route rule commands"
```

### Task 4: Documentation and Full Verification

**Files:**
- Modify: `README.md:107-145`
- Test: `tests/route.rs`, `tests/provider_store.rs`, `tests/cli.rs`

- [ ] **Step 1: Document runtime routing behavior**

Document rule commands, scan flags, selection precedence, fallback behavior,
and the fact that the current implementation does not add UI or failover.

- [ ] **Step 2: Run formatting and all tests**

Run:

```bash
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all commands pass with no warnings.

- [ ] **Step 3: Review the final diff and commit**

Run `git diff --check` and inspect the complete diff for credential leakage,
unintended config writes, and accidental changes to the P0-1 passthrough.

```bash
git add README.md
git commit -m "docs: document workspace routing"
```
