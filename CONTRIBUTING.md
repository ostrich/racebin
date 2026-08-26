# Contributing to Racebin

## Source placement

Racebin is one deployable application with explicit internal boundaries. Put
new code with the responsibility it implements rather than in a generic
utility or service module:

- `src/main.rs` remains a minimal executable entry point; process composition
  and startup belong in `src/app.rs`.
- Reusable paste, account, and instance rules belong in `src/pastes/`,
  `src/accounts/`, and `src/instance/`, respectively.
- Database selection, migrations, copy operations, and shared persistence
  infrastructure belong in `src/database/`.
- `src/http/` is a transport boundary. Handlers parse requests, resolve
  authentication, call domain operations, and serialize responses; reusable
  authorization and business rules do not live only in handlers.
- Cross-module Rust integration coverage belongs under `tests/integration/`.
- Frontend lifecycle infrastructure belongs in `web/src/app/` or
  `web/src/navigation/`; route components belong in `web/src/pages/`, shared
  controls in `web/src/components/`, and rich-text behavior in
  `web/src/rich-text/`.
- All browser requests go through named operations in `web/src/api/`. Pages
  and components do not call `fetch`, construct API URLs, or encode wire
  payloads themselves.
- Shared design primitives belong in the base style layers. Feature-specific
  layout belongs in a focused stylesheet under `web/src/styles/`; avoid
  page-local overrides of shared control geometry.

Do not add compatibility facades for obsolete internal paths. Move every
caller and remove the superseded module in the same coherent change. See
[Architecture](docs/architecture.md#source-tree-map) for the complete tree and
the reasoning behind these boundaries.

## API contract workflow

Racebin treats its HTTP API as an architectural boundary shared by the server,
the browser application, and third-party clients. Wire behavior is contract
first: a change is incomplete until the runtime, OpenAPI document, generated
wire types, first-party client, documentation, and tests agree.

For every API change:

1. Define or update the Rust request and response DTOs and their validation.
2. Update the corresponding OpenAPI operation, including errors, security,
   scopes, headers, media types, and behavioral descriptions.
3. Run `scripts/generate-api-contract.sh` and review the normalized
   `openapi/openapi.json` diff as part of the change.
4. Use the generated types in `web/src/api/generated.ts`; never edit that file
   manually.
5. Add or update a named function in the frontend API layer. UI components do
   not call `fetch`, construct API paths, or serialize wire DTOs directly.
6. Add focused contract and client tests plus real-stack coverage when the
   behavior crosses authentication, CSRF, ETag, idempotency, multipart, scope,
   or limited-read boundaries.
7. Update `docs/api.md` and user-facing help when callers need to understand
   the change.

CI regenerates both contract artifacts and rejects stale output. A changed
snapshot is therefore an intentional, reviewable API change rather than an
incidental side effect.
