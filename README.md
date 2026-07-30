# Racebin

Racebin is an API-first, self-hosted paste bin, file share, and URL shortener
with user accounts, invitation links, scoped API keys, and a Vanilla TypeScript
single-page application. The supported interface is `/api/v2`; the browser
application uses that same API.

## Features

- Public, unlisted, and owner-only pastes
- Text with light/dark syntax highlighting, URL pastes, titles, expiration, and burn-after-read
- Searchable language selection, automatic detection, and lazy-loaded uncommon grammars
- Multiple file uploads, individual downloads, and ZIP archives
- Session authentication with CSRF protection
- Invitation-based account creation and administrator controls
- User-owned API keys with explicit paste and administration scopes
- SQLite or PostgreSQL as the authoritative store
- Embedded responsive TypeScript application with no separate web server

Encryption, readonly/editable modes, the JSON database, server-rendered forms,
and the `/api/v1` interface are not supported.

## Run

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
files:

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
| `--title` | `Racebin` |
| `--no-file-upload` | disabled |
| `--max-file-size-mb` | `2048` |
| `--qr` | disabled |
| `--public-url` | unset; required for QR output |
| `--insecure-cookie` | disabled; use only for local HTTP |

See the [database guide](docs/database.md), [account guide](docs/accounts.md),
[API guide](docs/api.md), and [testing guide](docs/testing.md). The running
server also exposes a machine-readable route list at `/api/v2/openapi.json`.

## Databases

Startup selects the database from `--database-url` or
`RACEBIN_DATABASE_URL` and runs the migrations for that backend. SQLite URLs
and both `postgres://` and `postgresql://` URLs are supported. The data
directory continues to hold uploaded attachments when PostgreSQL is used, so
database backups alone are not sufficient for installations with files.

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
```

The ordinary test run uses temporary SQLite databases. Set
`RACEBIN_TEST_POSTGRES_URL` to a dedicated disposable PostgreSQL database to
run the same storage contract and copy tests against PostgreSQL. The test
suite drops that database's `public` schema; never point it at real data.

Racebin is available under the terms in [LICENSE](LICENSE) and
[LICENSE-BSD-3-Clause](LICENSE-BSD-3-Clause).
