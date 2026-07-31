# Architecture

This document describes how Racebin is structured, how its major components
interact, and where new behavior belongs. It reflects the current
implementation rather than a future design.

## Design goals

Racebin is designed as a small, self-hosted application with a simple
operational footprint. The architecture favors:

- one deployable server binary;
- a stable HTTP API shared by the browser application and other clients;
- SQLite for small installations and PostgreSQL for installations that need
  it;
- explicit authorization and transactional invariants in the backend;
- a frontend that can provide an application-like editing experience without
  requiring a separate JavaScript server; and
- portable, inspectable storage rather than mandatory external services.

The application is intentionally not divided into independently deployed
services. Its internal layers provide separation of concerns while remaining
part of one process.

## System overview

```mermaid
flowchart LR
    Browser["Browser<br>Svelte application"]
    Client["API client"]
    Actix["Actix Web server"]
    HTTP["HTTP handlers<br>/api/v1"]
    Service["Domain services"]
    Repository["SQLx repository"]
    Database[("SQLite or PostgreSQL")]
    Files[("Attachment files")]
    Assets["Embedded frontend assets"]

    Browser -->|JSON, multipart, cookies| Actix
    Client -->|JSON, multipart, bearer token| Actix
    Actix --> HTTP
    Actix --> Assets
    HTTP --> Service
    HTTP -->|file streaming| Files
    Service --> Repository
    Repository --> Database
    Service -->|attachment metadata| Database
```

The browser application never accesses storage directly. It uses the same
`/api/v1` endpoints exposed to external API clients. Actix serves that API,
the compiled frontend, attachment data, archives, and optional QR codes.

## Runtime composition

`src/main.rs` is the composition root. At startup it:

1. Dispatches a requested database or account CLI command, if present.
2. Validates server configuration.
3. Creates the data directory.
4. Opens the configured database and selects its backend.
5. Applies the matching SQLx migrations.
6. Purges expired records and orphaned attachment directories.
7. Starts an hourly expiration-cleanup task.
8. Constructs one shared `PasteService`.
9. Starts the Actix HTTP server with the configured worker count.

Actix owns the asynchronous runtime. The service and repository are cheap,
cloneable handles shared with each worker; SQLx owns the underlying connection
pool.

The server applies request logging, trailing-slash normalization, JSON and
query parsing limits, and common security headers at the application
boundary.

## Backend layers

The Rust backend is divided by responsibility:

| Layer | Location | Responsibility |
| --- | --- | --- |
| Process and configuration | `src/main.rs`, `src/args.rs` | Startup, environment and CLI configuration, server construction, periodic cleanup |
| HTTP transport | `src/http/` | Routing, request parsing, authentication extraction, status codes, cookies, uploads, downloads, and response serialization |
| Domain services | `src/services/` | Paste rules, visibility, ownership, validation, conversion, read limits, search, and transactional operations |
| Accounts and credentials | `src/account/` | Users, passwords, sessions, invitations, API keys, and scopes |
| Persistence | `src/repository.rs`, `src/repository/` | Backend selection, connection pooling, migrations, database copy, and shared storage primitives |
| Operator CLI | `src/cli/` | Account administration and database-copy commands |

HTTP handlers should remain transport adapters. Rules that must also hold for
future transports or CLI callers belong in a service or account operation,
not solely in a handler. Database-specific setup and migration selection
belong in the repository layer.

`PasteService` is currently the main application service. It owns a
`Repository` and accepts a `Principal` representing an anonymous request,
browser session, or API key. This keeps authorization decisions close to the
operations they protect.

## HTTP and API design

All application endpoints live below `/api/v1`. Routes are grouped by
resource:

- metadata and runtime configuration;
- sessions and accounts;
- pastes and rich-text conversion;
- attachments, archives, and QR output;
- user-owned API keys; and
- administrative users, invitations, API keys, and paste management.

The API uses JSON for ordinary requests and responses and multipart bodies for
file uploads. Errors have stable machine-readable codes and human-readable
messages. The machine-readable route description is exposed at
`/api/v1/openapi.json`.

Unknown API routes return JSON errors. Known browser routes receive the SPA
entry document, while unknown non-API paths return a 404. This explicit
allowlist prevents the SPA fallback from disguising arbitrary missing paths.

See [api.md](api.md) for the endpoint contract.

## Authentication and authorization

Racebin supports two authentication mechanisms:

- Browser sessions use an HTTP-only `racebin_session` cookie. The stored
  session contains a separate CSRF token that the frontend sends in the
  `X-CSRF-Token` header for mutations.
- API clients use bearer tokens. Each API key has an explicit set of scopes,
  and bearer-authenticated requests do not use browser CSRF protection.

Passwords are hashed with Argon2id. Session, invitation, and API-key secrets
are random values; only hashes are stored in the database. Token prefixes are
retained where an operator needs to identify a credential without recovering
its secret.

Authorization is enforced by both the HTTP and domain layers:

- transport helpers require authentication, CSRF validation, or an
  administrative scope;
- service validation enforces visibility and ownership;
- API-key scopes constrain individual operations; and
- transactional account operations prevent disabling or demoting the last
  enabled administrator.

Disabled users cannot authenticate, and disabling an account or changing its
password revokes its sessions. A forced password change limits the session to
the session and password endpoints until the password is replaced.

## Data model

The main relational entities are:

| Entity | Purpose and relationships |
| --- | --- |
| `users` | Account identity, password hash, role, enabled state, and forced-password-change state |
| `sessions` | Expiring browser credentials owned by users; deleted with their user |
| `invitations` | Expiring, revocable account invitations with creator and redeemer attribution |
| `api_keys` | Hashed bearer credentials, optionally owned by a user |
| `api_key_scopes` | Many-to-one scope assignments deleted with their API key |
| `pastes` | Text or rich-text content, owner, visibility, expiration, and read-limit state |
| `attachments` | Ordered attachment metadata owned by a paste |

Rich text is stored as a validated JSON document alongside a plain-text
representation. The document supplies formatting semantics; the text
representation supports conversion, search, API consumers, and graceful
plain-text behavior. The frontend uses Tiptap's ProseMirror document model,
but the backend validates the supported schema rather than accepting
arbitrary editor JSON.

Foreign keys implement ownership cleanup where possible. A deleted user
leaves their pastes intact with a null owner, while their sessions and
user-owned API keys are removed. Deleting a paste removes its attachment
metadata.

## Database abstraction

SQLx's `Any` driver provides the common query interface for SQLite and
PostgreSQL. `Repository::open` identifies the backend from the URL and
configures its pool:

- SQLite uses foreign-key enforcement, WAL mode, and a busy timeout.
- PostgreSQL uses its normal transactional and row-locking behavior.

Each backend has a parallel migration directory under `migrations/`. The
migration sets represent the same logical schema while allowing backend
syntax and identity behavior to differ.

Operations with concurrency-sensitive invariants use transactions. The
repository also provides a process-local write lock to serialize critical
write sequences consistently, while PostgreSQL operations use row locks where
appropriate. Examples include consuming a read-limited paste, redeeming an
invitation, preserving the last administrator, and assigning attachment
ordering.

The database-copy command migrates an empty destination, copies all
application rows transactionally, verifies attachment references and row
counts, and resets PostgreSQL identity sequences before committing.

See [database.md](database.md) for backend selection, backups, and migration.

## Attachment storage

Attachment metadata lives in the database, but bytes live below:

```text
<data-dir>/attachments/<paste-id>/<storage-key>
```

User-provided filenames are display metadata and are not used as filesystem
paths. Storage keys are generated identifiers, and path construction rejects
unsafe components.

Uploads are streamed into temporary files while enforcing per-request limits.
After all fields are valid, files are renamed to their final storage keys and
their metadata is inserted. Cleanup guards remove staged or promoted files
when a later step fails. Downloads re-check paste visibility and ownership
before opening a file.

The database and attachment directory therefore form one logical data set.
Backups must include both. PostgreSQL does not make attachment storage
external or replicated automatically.

## Frontend architecture

The browser interface is a Svelte 5 single-page application in `web/src`.
TypeScript is used for application code and component scripts.

Racebin does not use SvelteKit. Its small client router in `web/src/router.ts`
owns:

- route parsing and History API navigation;
- back/forward scroll restoration;
- explicit route-readiness coordination for asynchronously rendered pages;
  and
- unsaved-change guards for internal navigation and browser unloads.

`App.svelte` is the frontend composition root. It loads the current session,
applies route-level access control, and chooses the page component.
`Shell.svelte` owns the shared navigation and page frame. Pages compose
reusable controls from `web/src/components`.

Application-wide session and configuration state lives in a small Svelte
store. `requestApi` is the common API client and automatically adds JSON
headers and the current session's CSRF token to mutations.

Notable browser-side technologies are:

- **Tiptap/ProseMirror** for structured rich-text editing;
- **Highlight.js** for syntax highlighting and language detection;
- **Vite** for bundling and code splitting;
- **Vitest** with jsdom for unit and component tests; and
- **Playwright** for browser-level workflows.

Large optional features, including rich-text components and uncommon syntax
grammars, are loaded as separate JavaScript chunks. There is still one
application deployment: chunking reduces initial browser work rather than
creating separately deployed frontend services.

## Frontend build and embedding

The production build has two stages:

1. Vite compiles the Svelte application into `web/dist`.
2. Cargo builds the server after the frontend exists.

`build.rs` enumerates every file below `web/dist/assets`, determines its
content type, and generates Rust source containing `include_bytes!` entries.
The SPA entry document is included directly by `src/http/assets.rs`. The
resulting executable contains the HTML, CSS, JavaScript, and lazy-loaded
chunks needed by the browser.

This provides a single deployable binary and prevents runtime asset-version
mismatches. The tradeoff is that every frontend change requires rebuilding
the Rust binary. Development still uses Vite's local server and mocked API
responses for fast browser tests.

## Content flow

A typical paste creation follows this path:

1. A Svelte form collects text or a rich-text document and optional files.
2. The frontend sends paste metadata to `/api/v1/pastes`.
3. The HTTP handler resolves the principal and validates CSRF or API-key
   authentication.
4. `PasteService` validates content, visibility, language, expiration, read
   limits, ownership, and rich-text structure.
5. SQLx writes the paste in a transaction.
6. The frontend uploads attachments using the new paste ID.
7. The attachment handler streams files to staged paths, promotes them, and
   records their metadata.
8. The browser navigates to the paste view and consumes it through the same
   API available to other clients.

Reading through the consume endpoint atomically updates the read count and
removes a paste whose read limit has been reached. The response still
contains the consumed paste so the final permitted reader receives it.

## Background work and cleanup

Racebin performs a cleanup pass at startup and then hourly. It:

- deletes expired pastes;
- deletes expired sessions;
- removes old expired invitations;
- removes attachment directories belonging to deleted or unknown pastes.

This is an in-process task rather than a separate worker service. If Racebin
is stopped, cleanup resumes at the next startup.

## Deployment model

The normal production topology is:

```text
HTTPS reverse proxy
        |
Racebin system service
        |
        +-- SQLite file or PostgreSQL server
        |
        +-- local attachment data directory
```

The reverse proxy terminates TLS and forwards requests to Racebin. Secure
cookies are the default; `--insecure-cookie` exists only for local HTTP
development. `--public-url` is used when generating absolute QR destinations
and is otherwise not required for ordinary routing.

The repository includes a systemd unit and example environment file under
`packaging/`, plus an Arch `PKGBUILD`. They are packaging examples rather
than requirements of the runtime architecture.

Running multiple Racebin replicas would require shared attachment storage and
careful review of operations that currently rely on a process-local write
lock. The supported simple deployment is one Racebin process with multiple
Actix workers.

## Testing strategy

Tests are organized around architectural boundaries:

- repository unit tests cover storage helpers and query behavior;
- a shared backend contract runs against SQLite and PostgreSQL;
- concurrency tests exercise read limits, invitations, administrator
  invariants, and attachment ordering;
- migration tests verify startup and schema behavior;
- copy tests cover complete transfers, validation, rollback, and PostgreSQL
  sequence continuation;
- HTTP integration tests cover sessions, CSRF, API-key scopes, visibility,
  ownership, administration, and files;
- Vitest covers frontend routing, formatting, and components; and
- Playwright covers critical browser workflows with deterministic API
  fixtures.

PostgreSQL tests require a dedicated disposable database and reset its
`public` schema. See [testing.md](testing.md) for commands and safety details.

## Repository map

```text
src/
  account/              account, session, invitation, and API-key logic
  cli/                  operator commands
  http/                 Actix routes and transport concerns
  integration_tests/    backend, concurrency, migration, copy, and HTTP suites
  repository/           database-copy implementation and repository tests
  services/             paste domain model, validation, conversion, and service
web/
  src/components/       reusable Svelte controls
  src/pages/            route-level Svelte components
  e2e/                  Playwright browser workflows
  dist/                 compiled frontend embedded by Cargo
migrations/
  sqlite/               SQLite schema history
  postgres/             PostgreSQL schema history
docs/                   operator, API, testing, and architecture guides
packaging/               service configuration and package recipes
```

## Adding or changing functionality

Use the existing boundaries when extending Racebin:

- Add or change an API contract in `src/http`, but place reusable business
  rules in `src/services` or `src/account`.
- Keep SQLite and PostgreSQL migrations logically equivalent.
- Treat database rows and attachment files as a coordinated data set.
- Enforce authorization on the server even when the frontend hides a control.
- Add backend contract coverage for storage behavior shared by both
  databases.
- Add HTTP tests for authorization and error semantics.
- Add Playwright coverage when behavior depends on real browser layout,
  navigation, editing, or file interaction.
- Rebuild `web/dist` before compiling a production binary after frontend
  changes.

