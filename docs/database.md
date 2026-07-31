# Databases

Racebin supports SQLite and PostgreSQL through the same application and HTTP
interfaces. Select the backend with `--database-url` or
`RACEBIN_DATABASE_URL`.

## SQLite

SQLite is the default. When no database URL is set, Racebin opens:

```text
sqlite://<data-dir>/database.sqlite
```

For an explicit location:

```bash
racebin \
  --database-url 'sqlite:///var/lib/racebin/database.sqlite?mode=rwc' \
  --data-dir /var/lib/racebin
```

Racebin enables foreign keys, WAL mode, and a busy timeout on every pooled
SQLite connection.

## PostgreSQL

Create an empty database and provide its URL:

```bash
RACEBIN_DATABASE_URL='postgresql://racebin:password@localhost/racebin' \
RACEBIN_DATA_DIR=/var/lib/racebin \
racebin
```

Both `postgres://` and `postgresql://` schemes are accepted. Racebin applies
the PostgreSQL migration set during startup. Keep credentials out of shell
history and process arguments where possible; an environment file readable
only by the service account is preferable.

`data-dir` remains required operational state with either backend because
attachments are stored below `<data-dir>/attachments`. Metadata and
authentication records live in the selected database.

Folders and paste-to-folder assignments are relational metadata. Existing
pastes remain Uncategorized when the folder migration is first applied.

## Backups

For SQLite, stop Racebin and back up `database.sqlite` together with the
complete attachment directory. Include the WAL and shared-memory files if the
database is copied while it is open.

For PostgreSQL, take a consistent PostgreSQL backup and back up `data-dir`.
Coordinate the two backups when attachments must be restored to the exact
same point in time.

## Copy SQLite To PostgreSQL

Stop every Racebin process that can write to the source, then run:

```bash
racebin database copy \
  --from 'sqlite:///var/lib/racebin/database.sqlite' \
  --to 'postgresql://racebin:password@localhost/racebin' \
  --data-dir /var/lib/racebin
```

The destination must contain no Racebin application rows. The command:

1. Applies the appropriate migrations to both databases.
2. Reads a consistent source snapshot.
3. Verifies that every attachment referenced by metadata exists in
   `data-dir`.
4. Copies users, folders, sessions, invitations, API keys, pastes, and attachment metadata
   while preserving IDs.
5. Resets every PostgreSQL identity sequence.
6. Verifies table counts before committing the destination transaction.

Missing attachments, incompatible rows, a non-empty destination, or failed
verification abort the operation. Start Racebin with the PostgreSQL URL only
after the command succeeds.

Racebin supports its current migration history and forward migrations from
that schema. It does not import unrelated or historical application schemas.
