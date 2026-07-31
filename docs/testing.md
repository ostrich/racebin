# Testing

Run the local quality gates from the repository root:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Check, test, and build the embedded Svelte frontend separately:

```bash
cd web
npm ci
npx playwright install chromium-headless-shell
npm run check
npm test
npm run build
```

Vitest covers route, formatting, and Svelte component behavior in jsdom.
Playwright runs critical navigation, editor, highlighting, and dirty-form
workflows in Chromium against deterministic API fixtures.

Playwright also protects the shared UI geometry in two complementary ways:

- `layout-invariants.spec.ts` checks measurable contracts such as common
  content edges, stable filter boundaries, control heights, and the absence
  of horizontal page overflow.
- `visual-regression.spec.ts` compares representative desktop, mobile, light,
  and dark screens with reviewed snapshots.

When an intentional visual change requires new baselines, inspect the failed
images before updating them:

```bash
cd web
npm run test:e2e -- e2e/visual-regression.spec.ts --update-snapshots
```

Snapshot updates are review artifacts, not a substitute for investigating a
diff. Geometry invariants should express important alignment rules whenever
they can be stated without depending on pixels from a particular font or
operating system.

## PostgreSQL Tests

The normal Rust test run uses temporary SQLite databases. To exercise the
shared storage contract, concurrency behavior, and SQLite-to-PostgreSQL copy
path against PostgreSQL, set:

```bash
RACEBIN_TEST_POSTGRES_URL='postgresql://racebin:password@localhost/racebin_test' \
cargo test
```

Use a dedicated disposable database. The PostgreSQL test resets its state by
dropping and recreating the `public` schema. Never use a development,
staging, or production database.

The backend contract covers paste and folder behavior, accounts, sessions,
invitations, API keys, foreign-key cascades, expiration cleanup, and concurrency
invariants. Copy tests cover empty-destination enforcement, attachment
validation, transactional rollback, complete table transfer, and identity
sequence continuation. HTTP tests cover session and CSRF behavior, every API
key scope, ownership and administrative boundaries, scope delegation, and the
file lifecycle.

CI provisions a disposable PostgreSQL service and runs formatting, strict
Clippy, and the complete Rust test suite.
