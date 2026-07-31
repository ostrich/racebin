# Racebin

**Simple sharing for code, notes, and files.**

Racebin is a self-hosted place to create and share syntax-highlighted code,
rich-text documents, and attachments. Keep a paste private, share
it with an unlisted URL, or publish it for anyone to discover. Everything you
create stays manageable from your account.

## What you can do

- Share plain text with automatic language detection and syntax highlighting.
- Create rich-text pastes with headings, lists, links, quotes, code blocks, and
  other practical formatting.
- Move between plain text and rich text with a conversion preview.
- Add multiple files, download them individually, or bundle a paste and its
  attachments as a ZIP archive.
- Choose public, unlisted, or private visibility.
- Set an expiration time or limit how many times a paste can be read.
- Search, edit, and manage your pastes from one account.
- Organize saved pastes in private folders and move them in bulk.

## Built for self-hosting

- Invitation-based accounts and administrator controls
- User-owned API keys with explicit scopes
- SQLite by default, with PostgreSQL support when you need it
- A responsive browser application embedded in the Racebin binary
- A supported `/api/v1` used by both API clients and the browser application

## Quick start

```bash
cargo run --release -- \
  --data-dir ./racebin_data \
  --bind 127.0.0.1 \
  --port 7042 \
  --insecure-cookie
```

Create the first administrator:

```bash
racebin account create admin --admin --data-dir ./racebin_data
```

The account command prompts for a password. Passwords must contain at least 12
characters.

To use PostgreSQL, set a database URL while keeping `data-dir` for uploaded
attachments:

```bash
RACEBIN_DATABASE_URL='postgresql://racebin:password@localhost/racebin' \
RACEBIN_DATA_DIR=/var/lib/racebin \
racebin
```

## Configuration

All settings have equivalent `RACEBIN_*` environment variables.

| Option | Default |
| --- | --- |
| `--bind` | `0.0.0.0` |
| `--port` | `7042` |
| `--threads` | `2` |
| `--data-dir` | `racebin_data` |
| `--database-url` | `sqlite://<data-dir>/database.sqlite` |
| `--site-name` | `Racebin` |
| `--plain-home` | disabled; show a login-only anonymous homepage when enabled |
| `--attachments` | `true` |
| `--max-attachment-size-mb` | `2048` |
| `--qr-codes` | disabled |
| `--public-url` | unset; required for QR output |
| `--insecure-cookie` | disabled; use only for local HTTP |

See the [architecture overview](docs/architecture.md),
[database guide](docs/database.md), [account guide](docs/accounts.md),
[API guide](docs/api.md), and [testing guide](docs/testing.md). The running
server also exposes a machine-readable route list at `/api/v1/openapi.json`.

## Databases

Startup selects the database from `--database-url` or
`RACEBIN_DATABASE_URL` and runs the migrations for that backend. SQLite URLs
and both `postgres://` and `postgresql://` URLs are supported. The data
directory continues to hold uploaded attachments when PostgreSQL is used, so
database backups alone are not sufficient for installations with attachments.

To move an existing SQLite installation to an empty PostgreSQL database, stop
Racebin and run:

```bash
racebin database copy \
  --from 'sqlite:///var/lib/racebin/database.sqlite' \
  --to 'postgresql://racebin:password@localhost/racebin' \
  --data-dir /var/lib/racebin
```

The command migrates the destination schema, verifies that it contains no
application data, copies all records while preserving IDs and credentials,
checks attachment references and row counts, resets PostgreSQL identity
sequences, and commits the destination transaction only after verification.
Racebin must remain stopped for the duration of the copy. See
[docs/database.md](docs/database.md) for setup, backup, and migration details.

## Development

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cd web
npm ci
npm run check
npm test
npm run build
```

The ordinary test run uses temporary SQLite databases. Set
`RACEBIN_TEST_POSTGRES_URL` to a dedicated disposable PostgreSQL database to
run the same storage contract and copy tests against PostgreSQL. The test
suite drops that database's `public` schema; never point it at real data.
The browser interface is a Svelte 5 application compiled by Vite and embedded
in the Racebin binary. Frontend component tests use Vitest; critical browser
workflows use Playwright.

After changing Rust or frontend dependencies, run
`scripts/generate-third-party-licenses.sh` with `cargo-about` installed and
the frontend dependencies present.

Racebin is available under the [MIT License](LICENSE). Licenses for bundled
third-party components are collected in
[THIRD_PARTY_RUST_LICENSES.md](THIRD_PARTY_RUST_LICENSES.md) and
[THIRD_PARTY_FRONTEND_LICENSES.md](THIRD_PARTY_FRONTEND_LICENSES.md).
