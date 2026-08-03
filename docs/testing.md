# Development and testing

Racebin tests each architectural boundary independently and then exercises the
compiled application as a disposable stack. Commands in this guide are run
from the repository root unless a `cd web` is shown.

## Prerequisites

Install the Rust toolchain declared by the project, Node.js 24 with npm, and a
Playwright-compatible Chromium build. Install frontend dependencies and the
browser once per checkout:

```bash
npm --prefix web ci
(cd web && npx playwright install chromium)
```

The Playwright install command may require distribution packages when run on a
minimal Linux host; CI uses `playwright install --with-deps chromium`.

## Rust quality gates

Run the same formatting, linting, and test options used by CI:

```bash
cargo fmt -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
```

Without `RACEBIN_TEST_POSTGRES_URL`, the complete SQLite-backed suite runs and
the PostgreSQL runner returns without connecting. PostgreSQL setup is described
below.

The Rust suite includes:

- repository and schema tests;
- a shared SQLite/PostgreSQL backend contract for pastes, folders, accounts,
  sessions, invitations, password resets, API keys, expiration, and cascades;
- concurrency tests for consuming reads, invitation/reset redemption,
  administrator invariants, ETags, and attachment ordering;
- migration repeatability, checksum, and query-index tests;
- SQLite-to-PostgreSQL copy validation, rollback, attachment checks, and
  identity-sequence continuation;
- HTTP integration tests for sessions, CSRF, scopes, ownership,
  administration, rich text, and the attachment lifecycle; and
- OpenAPI structural and behavioral contract assertions.

## Frontend gates

The main frontend commands are:

```bash
cd web
npm run check
npm run test:unit
npm run test:e2e
npm run test:visual
npm run build
```

`npm test` runs unit, functional browser, and visual tests. The suites have
different responsibilities:

- Vitest covers route parsing, navigation transactions, access policy,
  readiness holds, superseded navigation, history/scroll state, dirty-form
  guards, query caching, formatting, API transport/resource behavior, and
  Svelte components in jsdom.
- Functional Playwright tests use deterministic API fixtures for editing,
  highlighting, folder/list behavior, caching, back/forward restoration,
  administration, responsive geometry, and accessibility-oriented controls.
- Visual Playwright tests compare reviewed desktop, mobile, light, and dark
  screenshots.
- Layout-invariant tests express measurable requirements such as common content
  edges, stable filter boundaries, shared control heights, and no horizontal
  page overflow.

When an intentional visual change requires new baselines, inspect the failed
images before updating them:

```bash
cd web
npm run test:visual -- --update-snapshots
```

Snapshot updates are review artifacts, not substitutes for diagnosing a diff.
Prefer a geometry assertion when the important behavior can be stated without
depending on a particular rendering host.

## API contract and frontend boundary

The Rust server generates the supported OpenAPI document. A normalized snapshot
and TypeScript wire types are committed, reproducible build artifacts:

```bash
cd web
npm run check:api
npm run check:api-boundary
```

`check:api` regenerates `openapi/openapi.json` and
`web/src/api/generated.ts` into a temporary directory and fails on any diff.
To intentionally update both artifacts after changing the Rust contract, run:

```bash
cd web
npm run generate:api
```

Review the normalized OpenAPI diff; never edit `generated.ts` manually.

`check:api-boundary` rejects direct frontend `fetch` calls outside the private
transport, use of the retired generic request interface, and page/component
imports that bypass the public resource layer. Add a named operation under
`web/src/api` for every new first-party API call.

## Disposable real-stack tests

The real-stack suite builds the production frontend and Rust binary, creates a
temporary SQLite data directory and administrator, starts Racebin on loopback,
and uses Chromium plus direct HTTP clients against the actual server:

```bash
cargo build --locked
cd web
npm run test:real
```

It covers the compiled frontend login/create/read path and real protocol
behavior including:

- browser cookies and CSRF rejection;
- bearer authentication and API-key scope boundaries;
- idempotent creation and replay headers;
- conditional updates and replacement ETags;
- multipart attachment creation; and
- final-read grants and post-consumption downloads.

The harness uses `${TMPDIR:-/tmp}/racebin-real-stack-playwright`, removes it on
teardown, and never opens the configured development or production database.
Do not run an unrelated service on its test port (`127.0.0.1:4174`).

## PostgreSQL tests

Create a dedicated disposable PostgreSQL database and set:

```bash
RACEBIN_TEST_POSTGRES_URL='postgresql://racebin:password@localhost/racebin_test' \
cargo test --locked --all-features
```

The test runner drops and recreates the database's `public` schema. Never point
it at development, staging, production, or a database containing unrelated
objects. The configured role must be able to create and drop objects in that
schema.

## CI workflow

GitHub Actions runs three jobs:

- **Rust** provisions PostgreSQL 18, checks retired naming, formatting, strict
  Clippy, and the complete SQLite/PostgreSQL suite.
- **Frontend** verifies generated API artifacts and the API boundary, builds
  and verifies committed `web/dist`, runs unit and functional Playwright tests,
  builds the Rust application, and runs the disposable real-stack suite.
- **Visual regression** runs after the frontend job succeeds in the pinned
  Playwright container.

The workflow uploads Playwright traces and failure details when a browser job
fails. A local pass without PostgreSQL does not replace the PostgreSQL-backed CI
run.
