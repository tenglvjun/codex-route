# Codex Provider Storage and CC Switch Import Design

**Status:** Design for review  
**Date:** 2026-09-02

## Context

`codex-route` is currently a read-only Rust CLI for indexing Codex rollout
files. It does not yet persist LLM provider configuration. `cc-switch` stores
providers in SQLite and uses a JSON `settings_config` payload for each app. For
the first provider milestone, `codex-route` will keep its own provider store and
will import only the Codex rows from a `cc-switch` database.

The two applications must remain operationally independent after import:
`codex-route` must be able to list and later route through an imported provider
when `cc-switch` is not running or is subsequently upgraded.

## Goals

- Add durable local storage for Codex provider configurations.
- Add `provider list`, `provider show`, and `provider import-cc-switch` CLI
  commands.
- Read the source `cc-switch.db` in read-only mode and select only rows whose
  `app_type` is `codex`.
- Preserve Codex configuration and unknown provider metadata without flattening
  it into a smaller custom schema.
- Make repeated imports deterministic and safe through source identity and an
  explicit conflict policy.
- Reject malformed or proxy-placeholder configurations without aborting valid
  rows in the same import.
- Keep the storage API independent from the CLI so a future HTTP gateway can
  reuse it.

## Non-goals

- No HTTP/SSE/WebSocket proxy implementation in this milestone.
- No writes to Codex `config.toml` or `auth.json`.
- No import of Claude, Gemini, Grok Build, or other `cc-switch` app types.
- No background synchronization or file watching.
- No attempt to reproduce the entire `cc-switch` provider UI, usage scripts,
  OAuth managers, or failover runtime.

## Approaches Considered

### A. Independent SQLite store (selected)

Use a new `codex-route.db` owned by this application. Open the source database
read-only, normalize each Codex row, and commit local changes in one
transaction. This matches `cc-switch`'s durability and transaction model while
avoiding runtime coupling to its schema or process.

### B. JSON provider file

Store a JSON array or object on disk. This is easy to inspect, but atomic
multi-record updates, concurrent invocations, schema migration, and conflict
handling would all need to be implemented separately. It also makes a future
gateway's concurrent reads/writes less predictable.

### C. Direct runtime use of `cc-switch.db`

Read the existing database whenever a provider is needed. This minimizes code,
but couples provider availability to a particular installation path and schema
version and prevents `codex-route` from owning local edits. It is rejected.

## Architecture

The implementation is split into four small layers:

```text
CLI (src/main.rs)
  -> ProviderStore (src/provider_store.rs)
       -> SQLite schema/migrations (src/provider_store.rs)
  -> CcSwitchImporter (src/cc_switch_import.rs)
       -> read-only source SQLite
       -> Codex normalizer/validator (src/codex_provider.rs)
```

The first version should use concrete structs and functions rather than a
general repository trait. A trait can be introduced when the future gateway
needs a second backend.

### Provider domain model

Define a serializable `Provider` focused on Codex but structurally compatible
with the useful part of `cc-switch`:

```rust
pub struct Provider {
    pub id: String,
    pub name: String,
    pub settings_config: serde_json::Value,
    pub website_url: Option<String>,
    pub category: Option<String>,
    pub created_at: Option<i64>,
    pub sort_index: Option<i64>,
    pub notes: Option<String>,
    pub icon: Option<String>,
    pub icon_color: Option<String>,
    pub meta: serde_json::Value,
    pub in_failover_queue: bool,
    pub is_current: bool,
    pub source: ProviderSource,
}
```

`settings_config` remains an opaque JSON object to avoid losing future Codex
fields. The importer may inspect it for validation, but storage and listing do
not rewrite its nested shape. `meta` is also preserved as raw JSON; fields such
as `apiFormat`, `providerType`, and `modelCatalog` remain available to the
future router.

`ProviderSource` is local metadata, not part of the imported `cc-switch` payload:

```rust
pub enum ProviderSource {
    Local,
    CcSwitch {
        source_id: String,
        source_updated_at: Option<i64>,
    },
}
```

The source identity makes a second import match a prior imported row even if
the provider name or local ID has changed.

### Local SQLite schema

Create the database lazily at the selected data directory. Use schema version
1 initially and `PRAGMA user_version` for future migrations.

```sql
CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    settings_config TEXT NOT NULL,
    website_url TEXT,
    category TEXT,
    created_at INTEGER,
    sort_index INTEGER,
    notes TEXT,
    icon TEXT,
    icon_color TEXT,
    meta TEXT NOT NULL DEFAULT '{}',
    in_failover_queue INTEGER NOT NULL DEFAULT 0,
    is_current INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'local',
    source_id TEXT,
    source_updated_at INTEGER,
    imported_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_source
    ON providers(source, source_id)
    WHERE source_id IS NOT NULL;
```

The source index gives imported records stable identity without forcing
user-created records to have a source ID. `settings_config` and `meta` must be
valid JSON when written. The store should serialize/deserialize with explicit
errors rather than silently replacing malformed values with `null`.

Provider endpoints are not needed to execute the active Codex configuration in
this milestone. They should remain in the opaque `meta` payload if already
present; a normalized endpoint table can be added with a later migration when
endpoint failover is implemented.

### Data directory and paths

Every provider command accepts an optional `--data-dir`. The default is a
platform-appropriate per-user configuration directory named `codex-route`.
Tests always pass a temporary directory. The source path for import is selected
as follows:

```text
--cc-switch-db
  -> default user config location/.cc-switch/cc-switch.db
```

The explicit source-path flag is important because `cc-switch` can use a custom
application directory and because users may maintain more than one profile.

On Unix-like systems, create the local database file with owner-only
permissions where the platform permits it. Never print `settings_config` or
credentials in import logs.

## CC Switch Import

### Source access

`CcSwitchImporter` opens the source with SQLite read-only flags and sets a short
busy timeout. It must not run migrations, writes, or `VACUUM` against the source
file. Before querying, verify that the `providers` table and the core columns
`id`, `app_type`, `name`, `settings_config`, and `meta` exist. The other
presentation columns (`website_url`, `category`, `created_at`, `sort_index`,
`notes`, `icon`, `icon_color`, `in_failover_queue`, and `is_current`) are
optional for compatibility with older `cc-switch` databases and default to
`null`/`false` when absent. A source without the core columns produces one fatal
import error with the source path and missing column, without exposing secrets.

The query is equivalent to:

```sql
SELECT id, name, settings_config, website_url, category,
       created_at, sort_index, notes, icon, icon_color, meta,
       in_failover_queue, is_current
FROM providers
WHERE app_type = 'codex'
ORDER BY COALESCE(sort_index, 999999), created_at ASC, id ASC;
```

This deliberately excludes all non-Codex application rows. If `is_current` is
available, it is carried into the import decision; import must not replace an
already-selected local provider unless the user explicitly requests it.

### Normalization and validation

For each row:

1. Parse `settings_config` and `meta` as JSON objects.
2. If `settings_config.config` is present, parse it as TOML using a structured
   parser, not string matching.
3. If `model_provider` is set, verify that the corresponding
   `model_providers.<id>` value is a table when the table exists.
4. Resolve the active `base_url` from the active provider table first, then the
   top-level compatibility field. Do not use an inactive provider table's URL.
5. Recognize credentials in `auth.OPENAI_API_KEY` and the active provider's
   `experimental_bearer_token`. Official Codex/OAuth rows may be keyless and
   remain valid.
6. Reject an otherwise third-party row whose only credential is
   `PROXY_MANAGED`, or whose active route is clearly a CC Switch local proxy
   placeholder. Do not reject a valid local relay merely because its URL is
   loopback when it has a real credential.
7. Do not filter solely on `wire_api`: Codex providers in `cc-switch` may be
   declared as Responses, OpenAI Chat, or Anthropic and the future proxy can
   bridge those protocols. The app-type filter is the authoritative scope.

Invalid rows are recorded in the import report and skipped; a malformed source
database, unreadable file, or failed local transaction is fatal.

### Identity and conflict policy

Imported records are matched by `(source = 'cc-switch', source_id = source
provider id)`. If no source match exists, the importer tries to retain the source
ID as the local `id`.

The command supports `--on-conflict`:

```text
skip     default; keep any existing local row and report it
replace  replace an existing cc-switch-origin row atomically
rename   on a local-ID collision, use a deterministic `ccswitch-<id>` ID
```

A local record is never silently overwritten by an imported record. When
`replace` encounters a local-ID collision, it also uses the deterministic
`ccswitch-<id>` namespace and records the original source ID. Re-importing the
same source row then updates that renamed record rather than creating another
copy.

The importer preserves existing local `is_current` and failover state on
replace. On first import, it may copy `in_failover_queue`; it must only set a
local current provider when the store has no current provider and the source row
was marked current. A future explicit `provider switch` command will own current
selection.

### Transaction and report

Read and validate all source rows first. Apply accepted changes in one local
SQLite transaction so a process interruption cannot leave half an import. The
result is a JSON report suitable for automation:

```json
{
  "source": "/path/to/cc-switch.db",
  "imported": 2,
  "replaced": 1,
  "renamed": 0,
  "skipped": 1,
  "rejected": [
    {"id": "broken", "reason": "invalid Codex config.toml"}
  ]
}
```

The report never includes `settings_config`, API keys, tokens, or raw SQLite
errors that may contain query values.

## CLI Surface

Extend the existing top-level `Command` enum with a nested provider command:

```text
codex-route provider list [--data-dir <dir>]
codex-route provider show <id> [--data-dir <dir>] [--reveal-secrets]
codex-route provider import-cc-switch
    [--data-dir <dir>]
    [--cc-switch-db <path>]
    [--on-conflict skip|replace|rename]
```

The existing `resolve` and top-level `list` commands remain unchanged. Provider
commands emit pretty JSON by default, matching the current CLI contract. The
default `show` output redacts known credential fields; `--reveal-secrets` is an
explicit opt-in for local debugging and must never be used by import logging.

## Error Handling

- Invalid CLI arguments continue to use exit code 2.
- Missing provider IDs continue to use the existing not-found semantics for
  provider `show` (a distinct provider error can map to code 3).
- Missing/unreadable source DB, incompatible schema, or local transaction errors
  use the existing operational failure code 4.
- Per-row JSON/TOML/config validation errors do not fail the whole command;
  they appear in `rejected` and the process exits successfully if the source
  was readable and at least the import operation completed.
- Credentials are never included in `Display` implementations, error strings,
  or logs.

## Testing Strategy

### Unit tests

- Schema creation, `user_version`, migration idempotence, and CRUD round trips.
- JSON round trip preserving unknown `settings_config` and `meta` fields.
- Read-only source query selects only `app_type = 'codex'`.
- Active provider URL extraction and credential extraction.
- Official keyless provider accepted; third-party placeholder rejected.
- Malformed JSON/TOML rows rejected without affecting valid rows.
- Re-import idempotence through `(source, source_id)`.
- `skip`, `replace`, and deterministic `rename` behavior.
- Current-provider preservation and first-import selection rules.

### CLI integration tests

Use temporary local and source databases, invoke the compiled binary, and
assert JSON reports and exit codes. Include a source fixture with Claude,
Gemini, official Codex, custom Responses, custom Chat, and malformed rows to
prove that only Codex rows are imported.

### Security/regression checks

- Import report and normal logs contain no API key/token values.
- Source database remains byte-for-byte unchanged after import.
- Interrupted or failed local transactions leave the previous provider set
  intact.
- Path resolution works with explicit paths on Unix, Windows, and macOS test
  targets.

## Implementation Sequence After Approval

1. Add SQLite and platform-path dependencies; introduce the local schema and
   `ProviderStore` with tests.
2. Add Codex normalization/validation helpers and source read-only importer.
3. Add nested CLI commands and JSON report/error mapping.
4. Add integration fixtures/tests and update README with paths, commands, and
   secret-handling behavior.

The HTTP gateway and provider selection logic should be a separate later
milestone built on `ProviderStore`, not mixed into this import/storage change.
