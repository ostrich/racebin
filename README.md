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

Clone the repository and start a local instance:

```bash
git clone https://github.com/ostrich/racebin.git
cd racebin
cargo run --release -- \
  --data-dir ./racebin_data \
  --bind 127.0.0.1 \
  --port 7042 \
  --insecure-cookie
```

Open [http://127.0.0.1:7042](http://127.0.0.1:7042) to explore Racebin. The
repository includes a prebuilt browser application, so the initial local run
requires only Rust. Node.js is required when modifying the frontend.

To create an administrator, stop Racebin and run:

```bash
cargo run --release -- account create admin --admin \
  --data-dir ./racebin_data
```

The account command prompts for a password of at least 12 characters. Start
Racebin again with the first command and sign in. This direct HTTP setup is for
local evaluation only.

For source builds, systemd installation, complete configuration, PostgreSQL,
Nginx and Caddy reverse proxies, TLS, upgrades, and troubleshooting, see the
**[production setup guide](docs/setup.md)**.

Additional references cover the [architecture](docs/architecture.md),
[databases](docs/database.md), [accounts](docs/accounts.md),
[HTTP API](docs/api.md), and [testing](docs/testing.md). The running server also
exposes generated OpenAPI documentation at `/api/v1/openapi.json`. Signed-in
users can open Help for API-key setup and installation-specific examples.

## Databases

SQLite is the default; PostgreSQL is optional. Startup selects the configured
backend and applies its migrations. Uploaded attachments remain in the data
directory with either backend, so database-only backups are incomplete. See
the [database guide](docs/database.md) for setup, backup, restore, and the
transactional SQLite-to-PostgreSQL copy command.

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
