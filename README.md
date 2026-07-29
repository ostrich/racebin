# Racebin

Racebin is an API-first, self-hosted paste bin, file share, and URL shortener
with user accounts, invitation links, scoped API keys, and a Vanilla TypeScript
single-page application. The supported interface is `/api/v2`; the browser
application uses that same API.

## Features

- Public, unlisted, and owner-only pastes
- Text, syntax metadata, URL pastes, titles, expiration, and burn-after-read
- Multiple file uploads, individual downloads, and ZIP archives
- Session authentication with CSRF protection
- Invitation-based account creation and administrator controls
- User-owned API keys with explicit paste and administration scopes
- SQLite as the single authoritative store
- Embedded responsive TypeScript application with no separate web server

Encryption, readonly/editable modes, the JSON database, server-rendered forms,
and the `/api/v1` interface are not supported.

## Run

```bash
cargo run --release -- \
  --data-dir ./racebin_data \
  --bind 127.0.0.1 \
  --port 8080
```

Create the first administrator:

```bash
racebin account create admin --admin --data-dir ./racebin_data
```

The account command prompts for a password. Passwords must contain at least 12
characters.

## Configuration

All settings have equivalent `RACEBIN_*` environment variables.

| Option | Default |
| --- | --- |
| `--bind` | `0.0.0.0` |
| `--port` | `8080` |
| `--threads` | `1` |
| `--data-dir` | `racebin_data` |
| `--title` | `Racebin` |
| `--default-expiry` | `never` |
| `--no-file-upload` | disabled |
| `--max-file-size-mb` | `2048` |
| `--qr` | disabled |

See [docs/api.md](docs/api.md) or the live `/api/v2/openapi.json` document for
the API.

## Migration

Startup performs a transactionally guarded schema migration. Public records
remain public; legacy private records become unlisted when they have no owner
and owner-only otherwise. Existing IDs, titles, content, files, owners, and
read statistics are retained.

The migration aborts before changing the database when it encounters encrypted
or readonly records. Back up `database.sqlite` and the attachment directory
before upgrading.

Racebin's original work is available under the [MIT License](LICENSE).
MicroBin-derived portions remain subject to the preserved
[BSD 3-Clause License](LICENSE-BSD-3-Clause).
