# Responses Route Design

**Status:** Design for review
**Date:** 2026-09-02

## Context

`codex-route` now owns a local SQLite provider store and can import Codex
provider rows from `cc-switch`. The stored configuration is not consumed by
Codex CLI directly, so an imported provider cannot yet be used without editing
Codex's live configuration. This milestone adds a local HTTP route that lets
Codex CLI send native Responses requests to `codex-route`, which then selects a
stored provider and forwards the request to its configured upstream.

The first route must remain intentionally narrow: it supports only the native
Codex Responses protocol and transparent SSE streaming. Chat Completions and
Anthropic protocol conversion are separate future work.

## Goals

- Add a blocking `route serve` CLI command for a local HTTP proxy.
- Bind only to loopback by default at `127.0.0.1:16729`.
- Route `POST /v1/responses` to the selected provider's upstream URL.
- Use the current stored provider by default and allow an explicit provider ID.
- Inject the provider credential while removing the client-supplied
  `Authorization` header.
- Preserve native Responses request bodies, status codes, useful headers, and
  streaming response bytes.
- Provide a small health endpoint for local readiness checks.
- Keep request/response bodies and credentials out of logs and errors.

## Non-goals

- No Chat Completions (`/v1/chat/completions`) support.
- No Anthropic or Gemini protocol conversion.
- No provider failover, retries, rate limiting, usage accounting, or request
  transformation in this milestone.
- No writes to Codex `config.toml` or `auth.json`.
- No non-loopback bind address or remote administration endpoint.

## Relationship To CC Switch

cc-switch uses a hybrid implementation: an Axum router and Tokio listener at
the service boundary, a manual Hyper HTTP/1.1 accept loop for wire-level
control, and Reqwest/Hyper clients for upstream calls. `codex-route` will use
the simpler Axum + Tokio + Reqwest path first. Manual Hyper connection handling
is not needed until a concrete compatibility issue requires preserving details
that Axum/Reqwest cannot preserve.

## CLI

Add a nested command:

```text
codex-route route serve
    [--data-dir <dir>]
    [--provider <id>]
    [--port <port>]
```

The server always binds to `127.0.0.1`; `--port` defaults to `16729`. The
provider database path is resolved exactly like the existing provider commands:
`<data-dir>/codex-route.db`, or the platform configuration directory when
`--data-dir` is omitted.

When `--provider` is omitted, each request resolves the provider with
`is_current = true`. An explicit provider ID is validated at startup and is
then used for every request. The server exits with a clear operational error
if the requested provider does not exist, if no current provider exists, or if
the selected provider has no usable Responses upstream URL or credential.

Startup writes a single readiness message to stderr with the loopback address
and selected provider mode. It never prints configuration payloads or secrets.
The process remains attached to the terminal until Ctrl-C, then shuts down the
listener gracefully.

## Request Flow

```text
Codex CLI
  POST http://127.0.0.1:16729/v1/responses
       |
       v
route handler
  - validate method/path
  - load selected Provider
  - require Responses wire_api
  - resolve active base_url and credential
  - strip client Authorization
  - stream request to upstream
       |
       v
Provider base_url + /responses
```

The handler accepts only `POST /v1/responses`. `GET /healthz` returns a small
JSON success response without loading or exposing provider credentials. Other
paths return `404`; other methods on the Responses path return `405`.

The upstream URL is formed by appending `responses` to the normalized active
`base_url`. A provider base URL may already end in `/v1` or another path; the
implementation must avoid double slashes and preserve any base path. Incoming
query parameters are copied to the upstream request.

The request body is streamed to Reqwest rather than buffered into an unbounded
in-memory `Vec`. Request headers are copied selectively, including content type,
accept, user-agent, request IDs, and Codex/OpenAI feature headers. Hop-by-hop
headers, `host`, `content-length`, and the client `authorization` header are
removed before sending. The route sets `Authorization: Bearer <credential>`
when a provider credential is available.

## Provider Selection And Configuration

Provider lookup reuses `ProviderStore` and never opens the cc-switch database
at request time. The active Codex TOML is parsed with the existing structured
helpers. The route accepts a provider when:

- `base_url` resolves from the active `model_providers.<id>` table or the
  top-level compatibility field;
- the active `wire_api` is absent or equals `responses` (case-insensitive);
- a usable `auth.OPENAI_API_KEY` or active
  `experimental_bearer_token` is present; and
- the configuration is not marked with `PROXY_MANAGED` or a known cc-switch
  proxy placeholder.

The route does not accept keyless ChatGPT OAuth as an upstream credential. A
keyless official provider can remain stored for future live-config support, but
this local upstream route returns a configuration error because it cannot
mint or refresh Codex OAuth credentials.

## Response Flow

Reqwest sends the request with a long-lived timeout suitable for model
generation. Once response headers arrive, the route returns the upstream status
code and a filtered copy of response headers. The body is exposed as an Axum
stream backed by `bytes_stream()`, preserving SSE event boundaries as received
by the HTTP client without parsing or rewriting event data.

Hop-by-hop headers and stale length/encoding headers are removed when the body
is streamed. `content-type: text/event-stream` and cache-control headers are
preserved. Non-streaming JSON responses are passed through the same path.

Connection failures before response headers produce a JSON `502` response with a
stable error code and no URL query, request body, or credential. Once upstream
headers have been sent, downstream disconnects terminate the stream; the route
does not attempt a second request.

## Error Contract

Use a small internal error type mapped to:

| Condition | HTTP response |
| --- | --- |
| Missing current/explicit provider | `503 provider_unavailable` |
| Missing base URL or credential | `503 provider_configuration_error` |
| Non-Responses provider | `501 responses_only` |
| Upstream connect/transport failure | `502 upstream_unavailable` |
| Unsupported path/method | `404` / `405` |
| Malformed request body | upstream-defined response; route does not parse it |

Error bodies use JSON with `error.code` and a short human-readable message.
They never include raw provider JSON, API keys, bearer tokens, or request
payloads.

## Components

- `src/route.rs`: CLI-independent server state, route construction, provider
  selection, header filtering, upstream request/response streaming, and error
  mapping.
- `src/main.rs`: `route serve` Clap arguments, Tokio runtime entry, provider
  store opening, and graceful shutdown wiring.
- `src/codex_provider.rs`: add an active `wire_api` extractor/validator while
  keeping existing base URL and credential helpers reusable.
- `Cargo.toml`: add `axum`, `tokio`, and `reqwest` with streaming/TLS features.
- `tests/route.rs`: unit and integration coverage with local mock upstream and
  loopback route server.
- `README.md`: document route startup and the required Codex `base_url`.

## Testing

- Unit-test URL joining, active `wire_api` extraction, credential selection,
  and hop-by-hop/authorization header filtering.
- Start a mock upstream on an ephemeral port and verify the route forwards a
  Responses JSON body, injects the stored key, removes the client key, and
  preserves upstream status/content type.
- Verify an SSE response arrives incrementally and unchanged.
- Verify explicit provider selection and current-provider selection.
- Verify missing provider, non-Responses provider, missing credential, and
  upstream connection failures map to the documented status/error codes.
- Verify the route binds to loopback and that `GET /healthz` is available.
- Run `cargo fmt --all -- --check`, `cargo test --locked`, and
  `cargo clippy --all-targets --all-features -- -D warnings`.

## Security And Operational Boundaries

The listener is loopback-only to prevent accidental LAN exposure. Provider
credentials come from the local provider database and are never accepted from
or echoed back to the client. The upstream URL is trusted configuration, but
must be an absolute `http` or `https` URL before a request is sent. The route
does not log headers, bodies, query strings, or provider configuration.
