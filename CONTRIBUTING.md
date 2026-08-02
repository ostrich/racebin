# Contributing to Racebin

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
