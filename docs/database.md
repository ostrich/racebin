# Databases and backups

Racebin supports SQLite and PostgreSQL through the same repository, domain, and
HTTP interfaces. Select the backend with `--database-url` or
`RACEBIN_DATABASE_URL`. Attachment bytes remain in the configured data
directory with either backend.

## SQLite

SQLite is the default and is recommended for small installations. Without an
explicit database URL, Racebin opens:

```text
sqlite://<data-dir>/database.sqlite?mode=rwc
```

An explicit path can be configured as follows:

```bash
racebin \
  --database-url 'sqlite:///var/lib/racebin/database.sqlite?mode=rwc' \
  --data-dir /var/lib/racebin
```

Racebin enables foreign keys, WAL mode, and a five-second busy timeout on every
pooled SQLite connection. It uses a process-local write lock for coordinated
multi-step mutations. The supported deployment remains one Racebin process;
multiple processes sharing one SQLite database are not a supported scaling
model.

## PostgreSQL

Create an empty database and provide its URL:

```bash
RACEBIN_DATABASE_URL='postgresql://racebin:password@localhost/racebin' \
RACEBIN_DATA_DIR=/var/lib/racebin \
racebin
```

Both `postgres://` and `postgresql://` schemes are accepted. Racebin applies the
PostgreSQL migration set at startup. Keep credentials out of process arguments
and shell history where practical; a protected service environment file is
preferable.

PostgreSQL stores relational metadata and authentication records only.
`RACEBIN_DATA_DIR` is still required because attachments live below
`<data-dir>/attachments`.

## Migrations

SQLite and PostgreSQL have parallel, append-only migration histories under
`migrations/sqlite` and `migrations/postgres`. Startup, account commands, and
database copy apply the migration set for the selected backend.

SQLx records applied migration versions and checksums. Do not edit an existing
migration after it has been applied; add a new migration instead. A checksum
mismatch stops startup rather than silently changing an established schema.
Take a coordinated backup before starting a newer binary because forward
migrations are not automatically reversible by an older binary.

Racebin supports its own current schema and forward migrations from its
committed history. It does not import unrelated or historical application
schemas.

## Backup and restore

The relational database and the entire attachment directory are one logical
data set. A usable restore needs matching copies of both.

### SQLite

The simplest reliable procedure is:

1. Stop Racebin.
2. Copy `<data-dir>/database.sqlite` and the complete
   `<data-dir>/attachments` tree.
3. Preserve ownership and private permissions.
4. Restart Racebin and check `/readyz`.

If an online SQLite backup is required, use SQLite's backup API or a tool such
as `sqlite3 .backup`; copying only the main database file while WAL mode is
active is not a consistent backup. Coordinate the attachment snapshot with the
database backup so newly committed metadata is not separated from its files.

### PostgreSQL

Take a consistent PostgreSQL backup with `pg_dump` or the site's normal backup
system and back up the data directory. Stop writes, or use storage/database
snapshot coordination, when an exact common point in time is required.

After restoring either backend, verify database readiness and exercise a known
attachment download. Racebin's startup cleanup removes orphaned attachment
directories; it cannot reconstruct a file missing from a database-backed
attachment record.

## Copy SQLite to PostgreSQL

The built-in copy command transfers Racebin's durable application records from
one database to an empty destination:

```bash
racebin database copy \
  --from 'sqlite:///var/lib/racebin/database.sqlite' \
  --to 'postgresql://racebin:password@localhost/racebin' \
  --data-dir /var/lib/racebin
```

Stop every Racebin process that can write to the source before running it. The
same data directory is used before and after the copy; attachment bytes are
verified but are not duplicated.

The command:

1. Opens both databases and applies their current migrations.
2. Refuses a destination containing Racebin application rows.
3. Reads a consistent source transaction.
4. Verifies that every attachment record references a copied paste and an
   existing file below `data-dir`.
5. Copies users, folders, sessions, password-reset tokens, invitations, API
   keys and scopes, pastes, and attachment metadata while preserving IDs.
6. Resets PostgreSQL identity sequences.
7. Verifies destination table counts before committing its transaction.

Short-lived operational records—authentication failures, idempotency records,
read receipts, and final-read grants—are deliberately not transferred. Missing
attachments, incompatible rows, a non-empty destination, or failed
verification aborts and rolls back the destination copy.

After a successful copy, configure `RACEBIN_DATABASE_URL`, start Racebin, check
`/readyz`, and verify account login plus representative paste and attachment
access before retiring the SQLite backup.
