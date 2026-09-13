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
flowchart TB
    subgraph Clients
        Browser["Browser<br/>Svelte application"]
        Client["CLI, desktop,<br/>uploader, or other<br/>API client"]
    end

    subgraph Binary["Racebin server binary"]
        direction TB
        Boundary["Actix HTTP boundary<br/>Routing and middleware<br/>Authentication"]
        Assets["Embedded browser app<br/>HTML, CSS, JavaScript<br/>Fonts"]
        Handlers["API handlers<br/>Request and response<br/>translation"]
        Domain["Domain services<br/>Authorization and<br/>business rules"]
        Persistence["Database infrastructure<br/>Transactions and<br/>SQLx queries"]

        Boundary -->|browser routes and assets| Assets
        Boundary -->|/api/v1| Handlers
        Handlers --> Domain
        Domain --> Persistence
    end

    Database[("SQLite or PostgreSQL")]
    Files[("Attachment data directory")]

    Browser -->|same-origin HTTP| Boundary
    Client -->|HTTP with bearer key| Boundary
    Persistence --> Database
    Handlers -->|upload and download streams| Files
    Domain -->|transactional promotion and cleanup| Files
```

The box around the server components is an important boundary: all of them are
compiled into or run within one process. They are modules, not separately
deployed services. The database and attachment directory together are the
persistent data set.

The browser application never accesses storage directly. After Actix serves
the embedded application, it uses the same `/api/v1` endpoints exposed to
external clients. Actix also serves attachment downloads, archives, and
optional QR codes after the domain layer authorizes the request.

### Request lifecycle

The system diagram shows ownership; the following sequence shows how a normal
API operation moves through those boundaries:

```mermaid
sequenceDiagram
    participant C as Browser or API client
    participant H as Actix boundary and handler
    participant S as Domain service
    participant R as Database infrastructure
    participant D as Database
    participant F as Attachment directory

    C->>H: HTTP request
    H->>H: Parse input and resolve principal
    H->>S: Typed operation
    S->>S: Validate authorization and invariants
    S->>R: Query or transactional write
    R->>D: Backend-specific SQL through SQLx
    D-->>R: Rows or commit result
    R-->>S: Domain data
    opt Upload or download includes attachment bytes
        H->>F: Stream staged upload or authorized download
    end
    opt Mutation promotes or removes attachment bytes
        S->>F: Coordinate file change with metadata transaction
    end
    S-->>H: Domain result or typed error
    H-->>C: JSON, file stream, or problem details
```

Not every request touches every participant. Static assets stop at the HTTP
boundary, metadata-only operations do not touch attachment storage, and CLI
commands may call repository or account operations without passing through
HTTP. The authorization and data invariants remain in the domain/account
layers so these alternate entry points cannot bypass them.

## Runtime composition

`src/main.rs` is deliberately minimal: it installs the asynchronous runtime
and delegates to the library application. `src/app.rs` is the composition
root. The application startup sequence:

1. Dispatches a requested database or account CLI command, if present.
2. Validates server configuration and creates the data directory.
3. Opens the configured database and selects its backend.
4. Applies the matching SQLx migrations.
5. Purges expired records and orphaned attachment directories.
6. Starts an hourly expiration-cleanup task.
7. Constructs one shared `PasteService`.
8. Starts the Actix HTTP server with the configured worker count.

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
| Process and composition | `src/main.rs`, `src/lib.rs`, `src/app.rs`, `src/args.rs` | Minimal executable entry point, reusable library boundary, configuration, server construction, and periodic cleanup |
| HTTP transport | `src/http/` | Routing, request parsing, authentication extraction, status codes, cookies, uploads, downloads, and response serialization |
| Paste domain | `src/pastes/` | Paste rules, visibility, ownership, validation, conversion, read limits, folders, attachments, search, and transactional operations |
| Accounts and credentials | `src/accounts/` | Users, passwords, sessions, invitations, API keys, scopes, administration, and throttling |
| Instance operations | `src/instance/` | Audit records and runtime settings |
| Database infrastructure | `src/database/` | Backend selection, connection pooling, migrations, database copy, and shared storage primitives |
| Operator CLI | `src/cli/` | Account administration, database copy, and OpenAPI export commands |

HTTP handlers should remain transport adapters. Rules that must also hold for
future transports or CLI callers belong in a paste, account, or instance
operation, not solely in a handler. Database-specific setup and migration
selection belong in the database layer.

`PasteService` is currently the main application service. It owns a
`Database` and accepts a `Principal` representing an anonymous request,
browser session, or API key. This keeps authorization decisions close to the
operations they protect.

### Error boundaries

Domain services and account operations return `DomainResult<T>`. A
`DomainError` retains its category, stable API code, and safe client-facing
message until the HTTP layer converts it to a problem-details response.
Transport handlers must use the common `domain_error` conversion rather than
reconstructing a status or code from an error string.

Raw string errors are limited to boundaries where they are the natural local
representation, including request parsing, rich-text parsing, operator CLI
commands, and repository startup or copy operations. Callers classify those
errors explicitly when they enter the domain layer. There is deliberately no
implicit `String` to `DomainError` conversion, so accidentally discarding a
typed error is a compile-time failure.

## HTTP and API design

The supported resource API lives below `/api/v1`. Conventional liveness and
readiness probes are also exposed as `/healthz` and `/readyz`. API routes are
grouped by resource:

- metadata and runtime configuration;
- sessions and accounts;
- pastes and rich-text conversion;
- attachments, archives, and QR output;
- user-owned API keys; and
- administrative users, invitations, API keys, and paste management.

The API uses JSON for ordinary requests and responses, accepts raw text and
forms for generic uploader compatibility, and uses multipart bodies for atomic
paste-and-file creation. Errors use RFC 9457-style problem details. The
Rust-generated OpenAPI 3.1 contract is exposed at `/api/v1/openapi.json`; a
normalized copy is committed at `openapi/openapi.json` for review and client
generation.

Unknown API routes return JSON errors. Known browser routes receive the SPA
entry document, while unknown non-API paths return a 404. This explicit
allowlist prevents the SPA fallback from disguising arbitrary missing paths.
Those browser routes are an implementation detail of the bundled client rather
than part of the public HTTP API contract.

See [api.md](api.md) for the endpoint contract.

## Authentication and authorization

Racebin supports two authentication mechanisms:

- Browser sessions use an HTTP-only `racebin_session` cookie. The stored
  session contains a separate CSRF token that the frontend sends in the
  `X-CSRF-Token` header for mutations.
- API clients use bearer tokens. Each API key has an explicit set of scopes,
  and bearer-authenticated requests do not use browser CSRF protection.

Racebin does not emit CORS headers and assumes its browser client is served from
the same origin. Cross-origin browser access, when deliberately required, is an
operator-owned reverse-proxy policy. Session cookies are HTTP-only,
`SameSite=Lax`, scoped to `/`, and secure except in explicit insecure-cookie
development mode.

Passwords are hashed with Argon2id. Session, invitation, and API-key secrets
are random values, and their hashes are used for authentication. Session and
API-key plaintext secrets are never retained. Active invitations additionally
retain their token so an administrator can copy the URL again; the token is
cleared when the invitation is redeemed or revoked.

Authorization is enforced by both the HTTP and domain layers:

- transport helpers require authentication, CSRF validation, or an
  administrative scope;
- service validation enforces visibility and ownership;
- API-key scopes constrain individual operations; and
- transactional account operations protect the single owner and prevent
  disabling or demoting the last enabled administrator.

The owner is a strict superset of the administrator role. Routine
administration can use browser sessions or explicitly scoped API keys.
Instance configuration, administrator-role changes, ownership transfer, and
audit access are browser-session-only; the most sensitive changes require a
password confirmation no older than ten minutes.

Disabled users cannot authenticate, and disabling an account or changing its
password revokes its sessions. A forced password change limits the session to
the session and password endpoints until the password is replaced.

## Data model

The main relational entities are:

| Entity | Purpose and relationships |
| --- | --- |
| `users` | Account identity, password hash, role, owner marker, enabled state, forced-password-change state, and last login |
| `sessions` | Expiring browser credentials and recent-authentication state owned by users; deleted with their user |
| `password_reset_tokens` | One-time, one-hour password recovery hashes created by administrators |
| `invitations` | Expiring, revocable account invitations with creator and redeemer attribution |
| `api_keys` | Hashed bearer credentials, optionally owned by a user |
| `api_key_scopes` | Many-to-one scope assignments deleted with their API key |
| `pastes` | Literal text or canonical Markdown, owner, visibility, expiration, revision, and read-limit state |
| `folders` | Private, flat organizational containers owned by users |
| `attachments` | Ordered attachment metadata owned by a paste |
| `idempotency_records` | Expiring create-request results used to make retries safe |
| `paste_read_receipts` | Expiring replay records for idempotent read requests |
| `paste_read_grants` | Short-lived capabilities for raw content and attachment downloads after limited reads |
| `auth_attempts` | Expiring authentication-failure records used for rate limiting |
| `instance_settings` | Singleton database-owned site identity, feature switches, and new-paste defaults |
| `audit_events` | Append-only snapshots of sensitive administrative activity |

Rich text is stored as canonical GitHub-Flavored Markdown. Comrak validates it
and derives sanitized HTML and plain text on demand. Tiptap's ProseMirror model
exists only while visual editing is active; it is never persisted or exposed as
the wire contract.

Foreign keys implement ownership cleanup where possible. A deleted user
leaves their pastes intact with a null owner, while their sessions and
user-owned API keys are removed. Deleting a paste removes its attachment
metadata.

Each owned paste may reference one folder. Folder identity is private to its
owner and does not affect paste visibility or URLs. Deleting a folder clears
the assignment rather than deleting its pastes.

## Database abstraction

SQLx's `Any` driver provides the common query interface for SQLite and
PostgreSQL. `Database::open` identifies the backend from the URL and
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
their metadata is inserted. Multipart pastes remain in a durable `pending`
state throughout this process and are invisible to every list, read, search,
download, and administrative aggregate until attachment completion commits.
Cleanup guards remove staged or promoted files when a later step fails.
Reconciliation waits an hour before treating an unreferenced file as abandoned
and rechecks paste existence immediately before removing a directory, so it
cannot mistake an in-flight promotion for crash debris. Downloads re-check
paste visibility and ownership before opening a file.

The database and attachment directory therefore form one logical data set.
Backups must include both. PostgreSQL does not make attachment storage
external or replicated automatically.

## Frontend architecture

The browser interface is a Svelte 5 single-page application in `web/src`.
TypeScript is used for application code and component scripts.

Racebin does not use SvelteKit. It has a deliberately application-specific
navigation runtime under `web/src/navigation`. The implementation is divided
by responsibility:

- `routes.json` is the canonical browser-route manifest shared with the Rust
  SPA fallback, while `routes.ts` provides typed matching, titles, and access
  metadata;
- `components.ts` is the exhaustive route-to-component registry and loads
  route pages on demand;
- `guards.ts` owns the active form's unsaved-change guard and the common
  discard prompt used by links, back/forward navigation, logout, and browser
  unloads;
- `scroll.ts` owns Racebin's namespaced History API state and scroll
  positions; and
- `runtime.ts` executes navigation transactions, access-policy redirects,
  page readiness, history updates, title updates, focus, and scroll
  restoration in a fixed order.

A navigation is resolved before it is published. Authentication,
administrator access, and forced-password-change redirects therefore happen
before a protected page can mount. Once the destination is published, the
route outlet holds its navigation while the page bundle loads and hands that
hold to the mounted page. A page may call `holdNavigation()` while its initial
data loads.
The transaction restores scroll and completes only after all holds are
released and Svelte has rendered. Holds and asynchronous policy decisions are
transaction-scoped, so stale work cannot complete or overwrite a newer
navigation.

Each browser history entry carries its own navigation index and scroll
position without replacing unrelated `history.state` fields. Internal
push/replace navigation starts at the top and moves focus to the page heading;
back/forward navigation restores the saved position without stealing focus.

`App.svelte` is the frontend composition root. It bootstraps application state,
provides the route access policy, and mounts the route outlet. Route identity
normally excludes query parameters, so filter and pagination changes update an
existing page rather than destroying it. The New Paste route deliberately
includes its query in component identity because folder query changes describe
a new form and a confirmed discard must reset the old draft. Route parameters
that identify a different resource also create a new page instance. The home
route reactively selects its authenticated editor, plain login, or public
landing child, so session changes do not depend on redundant navigation.
`Shell.svelte` owns the shared navigation and page frame. Pages compose
reusable controls from `web/src/components`.

Dirty forms use one lifetime-owned helper. Confirmation does not disarm a
mounted form: the component unregisters only when it is actually destroyed, or
explicitly disarms itself after a successful save. Replaceable page requests
use one generation-aware loader that suppresses stale successes and errors,
coordinates loading state, participates in navigation readiness, and owns
cancellation of both reactive and imperative reloads for the component
lifetime.

Application-wide session and configuration state lives in a small Svelte
store. Initial application bootstrap, session refresh, capabilities refresh,
and language refresh are separate operations so a secondary metadata failure
cannot turn a successful login into an apparent authentication failure. Stable
session-expiration problem types reconcile the global session and redirect to
login; ordinary credential errors do not.

The browser API boundary is divided into generated wire types,
resource-specific normalization, named resource operations, and one private transport under
`web/src/api`. The transport alone performs network requests and owns JSON and
multipart serialization, CSRF, conditional and idempotency headers, protocol
response headers, problem-details errors, and query invalidation. Pages call
named operations and do not construct API paths or wire requests.

The committed, normalized OpenAPI snapshot is generated from the Rust routing
contract, and the TypeScript wire types are generated from that snapshot. CI
regenerates both artifacts, rejects stale output, and rejects direct frontend
network access outside the transport. This makes an API change a coordinated
change to the runtime, contract, generated types, resource client, and tests.

The query cache is kept separate from navigation: it deduplicates and retains
resource reads, invalidates them after mutations, and lets list pages render
cached data while revalidating. The shared page loader prevents an older
response or error from replacing a newer query, while navigation readiness
determines only when the new page is structurally ready for focus and scroll
restoration. Collection pages own their page envelope, totals, pagination, and
selection; row components emit mutations instead of maintaining shadow copies.

Growing collection endpoints return a common page envelope and perform search,
filtering, ordering, counting, and slicing in SQL. The browser stores those
parameters in the route query string and renders the shared pagination control.
Administrative rows carry the display data they require, such as an owner
username, rather than making the UI fetch an unbounded related collection and
join it in memory. Dashboard totals come from a purpose-built aggregate summary
endpoint. Small navigation collections, currently folders, are capped and
publish their limit through server capabilities.

Notable browser-side technologies are:

- **Tiptap/ProseMirror** for structured rich-text editing;
- **Comrak** for server-side CommonMark/GFM validation and rendering;
- **Highlight.js** for syntax highlighting and language detection;
- **Inter 4.1** as a bundled variable font for consistent layout across hosts;
- **Vite** for bundling and code splitting;
- **Vitest** with jsdom for unit and component tests; and
- **Playwright** for browser-level workflows.

Large optional features, including rich-text components and uncommon syntax
grammars, are loaded as separate JavaScript chunks. There is still one
application deployment: chunking reduces initial browser work rather than
creating separately deployed frontend services.

### Styling system

`web/src/style.css` is the stylesheet manifest. It declares an explicit CSS
cascade order: `reset`, `tokens`, `foundations`, `components`, `utilities`,
then `overrides`. Import order within a layer cannot accidentally outrank a
later architectural layer.

1. `web/src/styles/base.css` supplies the reset, typography, and document
   frame.
2. `web/src/styles/tokens.css` defines semantic colors, spacing, control
   geometry, radii, page dimensions, sticky offsets, and stacking levels.
   Dark mode changes these tokens rather than restyling individual components.
3. `web/src/styles/primitives.css` defines reusable layout and interaction
   primitives such as stacks, clusters, headings, buttons, labeled fields,
   and the control surface used by both native fields and composite widgets.
4. `web/src/styles/shell.css`, `administration.css`, `paste-view.css`, and
   `paste-controls.css` define feature-owned page compositions in explicit
   cascade order.
5. `web/src/styles/rich-text.css`, `folder-responsive.css`, and
   `paste-library.css` contain focused feature styling.
6. `web/src/styles/utilities.css` contains the small, documented set of
   utilities allowed to override component declarations.
7. `web/src/styles/responsive.css` applies the final cross-feature responsive
   adaptations in the `overrides` layer.

New UI should use semantic tokens and existing primitives before adding a
component-specific rule. Foundations own baseline geometry and appearance;
component rules describe real variants instead of cancelling foundation
properties. Components own their internal layout, while pages own only the
arrangement between components. Fixed dimensions and sticky offsets must come
from tokens when they participate in shared alignment. This keeps layout
behavior consistent and prevents page-specific overrides from becoming a
second design system. Primary route pages use the shared `page-layout` and
`page-heading` contract so their title origin, eyebrow rhythm, and
first-content boundary remain consistent even when the content itself uses a
sidebar.

`npm run check:css` applies standards linting and project-specific boundaries:
literal colors and theme selectors are confined to the token sheet,
`!important` is confined to utilities, duplicate declarations and conflicting
property ownership for the same selector are rejected, and the global layer
order is verified. Visual tests exercise the
representative paste workspace in automatic, light, and dark themes at desktop
and mobile sizes. Functional tests additionally assert semantic invariants,
such as a primary action retaining its filled accent treatment.

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

1. A Svelte form collects plain text or canonical GFM Markdown and optional files.
2. The frontend sends one JSON or multipart request to `/api/v1/pastes`.
3. The HTTP handler resolves the principal and validates CSRF or API-key
   authentication.
4. `PasteService` validates content, visibility, language, expiration, read
   limits, ownership, and Markdown structure.
5. The multipart parser streams files to staging while calculating their
   digests and enforcing configured size, field, and attachment-count limits.
6. SQLx records the paste as pending, independently records any idempotency
   state, and records attachment metadata after promotion.
7. Completion atomically makes the paste visible and completes any keyed retry
   record. Equivalent completion attempts are successful no-ops.
8. The browser navigates to the paste view and reads it through the same
   API available to other clients.

`GET /api/v1/pastes/{id}` is metadata-only.
`POST /api/v1/pastes/{id}/reads` atomically updates the read count. Authenticated
browser-session owners are exempt when viewing their own pastes, so routine
owner previews do not affect analytics or consume read limits. A final read
tombstones the paste instead of immediately deleting its row and issues a
15-minute capability for its files; cleanup later removes the tombstone and
storage. Owner and administrator source reads do not consume the paste.
Revisions and ETags protect update and delete operations from lost updates.

## Background work and cleanup

Racebin performs a cleanup pass at startup and then hourly. It:

- deletes expired pastes;
- deletes abandoned pending paste creations and their attachment data;
- deletes consumed paste tombstones after the attachment-grant window;
- deletes expired idempotency records, read receipts, and follow-up read grants;
- deletes expired sessions and password-reset tokens;
- deletes stale authentication-attempt records;
- removes old expired invitations;
- removes stale upload-staging and unreferenced promoted files after a grace
  period; and
- removes attachment directories belonging to deleted or unknown pastes after
  rechecking current database state.

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

The reverse proxy terminates TLS and forwards requests without rewriting paths.
Racebin remains a single service even when it uses PostgreSQL; attachments
continue to live in the configured data directory.

The repository includes a systemd unit and example environment file under
`packaging/`, plus an Arch `PKGBUILD`. They are packaging examples rather
than requirements of the runtime architecture.

Running multiple Racebin replicas would require shared attachment storage and
careful review of operations that currently rely on a process-local write
lock. The supported simple deployment is one Racebin process with multiple
Actix workers.

See the [setup guide](setup.md) for build and installation instructions,
configuration, systemd, Nginx and Caddy examples, trusted-proxy behavior, TLS,
upgrades, and troubleshooting.

## Testing strategy

Tests are organized around architectural boundaries:

- database unit tests cover storage helpers and query behavior;
- a shared backend contract runs against SQLite and PostgreSQL;
- concurrency tests exercise read limits, invitations, one-time password
  resets, administrator invariants, and attachment ordering;
- migration tests verify startup and schema behavior;
- copy tests cover complete transfers, validation, rollback, and PostgreSQL
  sequence continuation;
- HTTP integration tests cover sessions, CSRF, API-key scopes, visibility,
  ownership, administration, and files;
- Vitest covers frontend routing, formatting, and components; and
- Playwright covers critical browser workflows with deterministic API
  fixtures and exercises authentication, CSRF, API-key scopes, idempotency,
  ETags, multipart uploads, and final-read grants through a disposable real
  server.

PostgreSQL tests require a dedicated disposable database and reset its
`public` schema. See [testing.md](testing.md) for commands and safety details.

## Source-tree map

```text
src/
  accounts/             identity, session, invitation, API-key, and administration logic
  app.rs                server composition and process lifecycle
  cli/                  operator commands
  database/             backend selection, migrations, copy, and persistence tests
  http/                 Actix routes, resource contracts, OpenAPI, and transport concerns
  instance/             audit and runtime settings
  lib.rs                reusable crate boundary
  main.rs               minimal executable entry point
  pastes/               paste domain model, authorization, operations, and rich text
tests/
  integration/          backend, concurrency, migration, copy, and HTTP suites
web/
  src/app/              shared session, state, cache, notices, and preferences
  src/api/              the enforced HTTP client boundary and generated types
  src/components/       reusable Svelte controls
  src/navigation/       routes, guards, history/scroll, and navigation runtime
  src/pages/            route-level Svelte components
    admin/              administrative route components
  src/rich-text/        rich-text editor, viewer, and paste normalization
  src/styles/           ordered design layers and feature-owned layouts
  e2e/                  Playwright browser workflows
  dist/                 compiled frontend embedded by Cargo
openapi/                normalized generated API contract
migrations/
  sqlite/               SQLite schema history
  postgres/             PostgreSQL schema history
docs/                   operator, API, testing, and architecture guides
packaging/               service configuration and package recipes
scripts/                 reproducibility, naming, and architecture checks
```

## Adding or changing functionality

Use the existing boundaries when extending Racebin:

- Add or change an API contract in `src/http`, but place reusable business
  rules in `src/pastes`, `src/accounts`, or `src/instance`.
- Keep SQLite and PostgreSQL migrations logically equivalent.
- Treat database rows and attachment files as a coordinated data set.
- Enforce authorization on the server even when the frontend hides a control.
- Add backend contract coverage for storage behavior shared by both
  databases.
- Add HTTP tests for authorization and error semantics.
- Update the OpenAPI contract and regenerate its snapshot and TypeScript wire
  types for every API change; frontend callers go through a named API resource.
- Add Playwright coverage when behavior depends on real browser layout,
  navigation, editing, or file interaction.
- Rebuild `web/dist` before compiling a production binary after frontend
  changes.
