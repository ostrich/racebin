# Accounts and access

Racebin uses local username/password accounts for browser access and scoped API
keys for automation. Registration is invitation-only; there is no public
self-registration or email-recovery service.

## Browser sessions

Passwords are stored as Argon2id hashes. A normal browser session expires after
12 hours. Selecting **Keep me signed in** creates a 30-day session and gives the
cookie a matching lifetime. Session cookies are HTTP-only, `SameSite=Lax`,
scoped to `/`, and secure unless the explicit local-development option is
enabled.

Every browser session has a separate CSRF token. The bundled frontend sends it
with state-changing API requests. Changing a password, disabling an account, or
using a password-reset link revokes that user's browser sessions. Logging out
revokes only the current session.

The administration UI reports active database sessions, not physical devices.
Racebin does not currently store IP address, browser, or device metadata for a
session, so multiple logins from the same device are still separate sessions.

Repeated authentication failures are limited in a 15-minute window. Login is
limited per account and client address; invitation redemption and password
reset are limited per client address. Rate-limited HTTP responses include
`Retry-After`. Behind a reverse proxy, configure trusted proxy addresses so the
client-address limit is meaningful; see [setup.md](setup.md).

## Invitations

Administrators create 24-hour invitations from `/admin`. A recipient chooses a
username and password when redeeming the one-use link. The administration page
shows whether an invitation is active, redeemed, revoked, or expired, including
the redeeming username when applicable.

Racebin retains an active invitation's token so its full URL can be copied
again. The recoverable token is cleared when the invitation is redeemed or
revoked. Invitations created before recoverable invitation URLs were supported
must be replaced if their original URL was lost. Expired invitation records are
retained for 30 days and then removed by cleanup.

## Passwords and recovery

Users change their own password under `/account`. Administrators can create a
one-use password-reset link from a user's administration page. A new reset link
replaces any existing link for that user, expires after one hour, and is stored
only as a hash. Creating the link does not sign the user out; successfully
resetting the password revokes all existing sessions.

Disabled users cannot sign in or use password-reset links. Racebin prevents the
last enabled administrator from being disabled or demoted.

## Roles, API keys, and scopes

The `user` role manages its own pastes, folders, password, and API keys. The
`admin` role also receives administrative access. Authorization is still
checked per operation: API keys carry explicit scopes rather than inheriting
unrestricted browser privileges.

Users create keys under `/account`. The full token is shown once; Racebin stores
only its hash and display prefix. Disabling a user also makes their keys
unusable. Demoting an administrator disables any of their keys that contain an
administrative scope.

The supported scopes and descriptions are published by
`GET /api/v1/capabilities`. See [api.md](api.md) for the authentication model
and client examples.

## Administration UI

The user list shows account state, role, last login, active sessions, API-key
counts, paste count, and total stored bytes including attachments. A user's
detail page supports:

- enabling or disabling the account;
- assigning the user or administrator role;
- creating and copying a password-reset link;
- revoking every browser session;
- revoking every API key; and
- opening the administrative paste list filtered to that owner.

The administration home also manages invitations and all API keys. Paste
administration provides owner-aware search and filtering.

## Operator CLI

The permanent bootstrap and recovery commands operate directly on the selected
database:

```bash
racebin account create USERNAME --admin --data-dir /var/lib/racebin
racebin account list --data-dir /var/lib/racebin
racebin account password USERNAME --data-dir /var/lib/racebin
racebin account disable USERNAME --data-dir /var/lib/racebin
racebin account enable USERNAME --data-dir /var/lib/racebin
racebin account role USERNAME user --data-dir /var/lib/racebin
racebin account role USERNAME admin --data-dir /var/lib/racebin
```

For PostgreSQL, pass `--database-url postgresql://...` to the command or set
`RACEBIN_DATABASE_URL`. Without a database URL, the CLI opens
`<data-dir>/database.sqlite`. `--data-dir` still identifies attachment storage
and should match the server configuration.

`create` and `password` prompt without echo. For non-interactive provisioning,
pass `--password-file PATH`; Racebin reads the file's contents, removes a
trailing newline, and does not retain the file. Protect and remove that file in
the calling automation.

The CLI applies pending migrations before performing an account operation. Back
up production state before running a newer binary against it, just as for a
normal server upgrade.
