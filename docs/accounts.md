# Accounts

Racebin uses username/password accounts for browser access. Passwords are
stored as Argon2id hashes. Browser sessions expire after 12 hours, or after 30
days when "Remember me" is selected. Disabling an account or changing its
password revokes its sessions.

Administrators create 24-hour invitations from `/admin`. Invitation recipients
choose their own username and password. Active invitation URLs can be copied
again from the administration page. Racebin clears the recoverable token when
an invitation is redeemed or revoked; invitations created before this support
was added must be replaced if their original URL was lost. Users manage their
password and API keys at `/account`.

The administration user list shows account status, last login, active
sessions, API keys, paste count, and stored bytes. Each user has a detail page
where an administrator can change access, revoke sessions or API keys, and
create a one-time password reset link. Reset links expire after one hour and
only their hashes are stored. Successfully using a reset link invalidates all
of that user's browser sessions; creating the link does not sign them out.

The permanent account recovery and bootstrap commands operate directly on the
configured database:

```bash
racebin account create USERNAME --admin --data-dir /var/lib/racebin
racebin account list --data-dir /var/lib/racebin
racebin account password USERNAME --data-dir /var/lib/racebin
racebin account disable USERNAME --data-dir /var/lib/racebin
racebin account enable USERNAME --data-dir /var/lib/racebin
racebin account role USERNAME user --data-dir /var/lib/racebin
```

For PostgreSQL, pass `--database-url postgresql://...` to each account command
or set `RACEBIN_DATABASE_URL`. Without a database URL, the command uses
`<data-dir>/database.sqlite`. `--data-dir` still selects attachment storage.

`create` and `password` prompt without echo. For automation, pass
`--password-file PATH`; the file is read once and is not retained by Racebin.
