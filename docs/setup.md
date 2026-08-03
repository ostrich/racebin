# Building and Setting Up Racebin

This guide covers a production-style Racebin installation on one Linux host.
The supported topology is one Racebin process behind an HTTPS reverse proxy,
using SQLite or PostgreSQL for relational data and a local data directory for
attachments.

```text
Internet → HTTPS reverse proxy → Racebin on 127.0.0.1:7042
                                  ├─ SQLite or PostgreSQL
                                  └─ /var/lib/racebin/attachments
```

Racebin is a single binary with its compiled Svelte application embedded. It
does not require a separate JavaScript server at runtime.

## Requirements

Building from source requires:

- Rust 1.88 or newer;
- Node.js 24 and npm;
- Git; and
- a C/C++ build toolchain suitable for Rust dependencies.

PostgreSQL is optional. SQLite support is compiled into Racebin and is the
recommended default for a small installation.

## Build from source

Build the frontend before compiling Rust because `build.rs` embeds the files
from `web/dist` into the executable:

```bash
git clone https://github.com/ostrich/racebin.git
cd racebin
npm --prefix web ci
npm --prefix web run build
cargo build --release --locked
```

The resulting executable is `target/release/racebin`. It contains the HTTP
server, operator commands, database migrations, and browser application.

### Arch Linux package

The repository includes a VCS package under `packaging/arch`:

```bash
cd packaging/arch
makepkg -si
```

The package installs `/usr/bin/racebin`, `/usr/lib/systemd/system/racebin.service`,
and `/etc/racebin.conf`. Its service uses Arch's `http` account. Review the
configuration before enabling it.

### Generic Linux installation

Install the binary and create a dedicated unprivileged account:

```bash
sudo install -Dm0755 target/release/racebin /usr/local/bin/racebin
sudo useradd --system --home-dir /var/lib/racebin \
  --shell /usr/bin/nologin --user-group racebin
sudo install -d -o racebin -g racebin -m0700 /var/lib/racebin
sudo install -Dm0640 -o root -g racebin \
  packaging/racebin.conf.example /etc/racebin.conf
```

Use the packaged unit as a security-hardening reference, but change its
Arch-specific account and binary path. A portable starting point is:

```ini
[Unit]
Description=Racebin API-first paste bin
After=network.target

[Service]
User=racebin
Group=racebin
WorkingDirectory=/var/lib/racebin
StateDirectory=racebin
StateDirectoryMode=0700
UMask=0077
EnvironmentFile=/etc/racebin.conf
ExecStart=/usr/local/bin/racebin
Restart=on-failure
KillSignal=SIGINT

NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
PrivateDevices=true
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6

[Install]
WantedBy=multi-user.target
```

Save it as `/etc/systemd/system/racebin.service`, then run
`sudo systemctl daemon-reload`. The more extensive hardening in
`packaging/racebin.service` can be copied when the target distribution supports
all of its directives.

## Configure Racebin

Racebin accepts command-line options and equivalent `RACEBIN_*` environment
variables. `/etc/racebin.conf` is a systemd environment file, so quote values
containing spaces and keep it readable only by root and the service group. Run
`racebin --help` for the executable's current option list.

| Option | Environment variable | Default |
| --- | --- | --- |
| `--bind` | `RACEBIN_BIND` | `0.0.0.0` |
| `--port` | `RACEBIN_PORT` | `7042` |
| `--threads` | `RACEBIN_THREADS` | `2` |
| `--data-dir` | `RACEBIN_DATA_DIR` | `racebin_data` |
| `--database-url` | `RACEBIN_DATABASE_URL` | SQLite at `<data-dir>/database.sqlite` |
| `--public-url` | `RACEBIN_PUBLIC_URL` | unset |
| `--trusted-proxies` | `RACEBIN_TRUSTED_PROXIES` | unset |
| `--site-name` | `RACEBIN_SITE_NAME` | `Racebin` |
| `--plain-home` | `RACEBIN_PLAIN_HOME` | `false` |
| `--attachments` | `RACEBIN_ATTACHMENTS` | `true` |
| `--max-attachment-size-mb` | `RACEBIN_MAX_ATTACHMENT_SIZE_MB` | `2048` |
| `--qr-codes` | `RACEBIN_QR_CODES` | `false` |
| `--insecure-cookie` | `RACEBIN_INSECURE_COOKIE` | `false` |

`RACEBIN_ATTACHMENTS=false` disables new uploads; it does not erase existing
attachment data or prevent authorized downloads. Racebin also enforces fixed
limits published by `/api/v1/capabilities`: 200 title characters, 2 MiB of text
or rich-text input, 32 attachments per paste, 100 pastes per bulk move, and a
maximum list page size of 100. The configurable attachment limit applies to
each file and to the combined file bytes in one multipart request.

`RACEBIN_PLAIN_HOME=true` gives anonymous visitors a minimal login-oriented
home page. It does not disable public paste URLs or `/explore`.

A typical SQLite production configuration is:

```ini
RACEBIN_BIND=127.0.0.1
RACEBIN_PORT=7042
RACEBIN_DATA_DIR=/var/lib/racebin
RACEBIN_PUBLIC_URL=https://paste.example.com
RACEBIN_TRUSTED_PROXIES=127.0.0.1,::1
RACEBIN_SITE_NAME=Racebin
RACEBIN_ATTACHMENTS=true
RACEBIN_MAX_ATTACHMENT_SIZE_MB=2048
```

Do not enable `RACEBIN_INSECURE_COOKIE` in an HTTPS deployment. It exists only
for direct local HTTP development. `RACEBIN_PUBLIC_URL` supplies the canonical
external origin for generated invitation, reset, paste, attachment, archive,
and QR links. Without it, canonical base URLs are omitted from capabilities;
QR output requires it.

`RACEBIN_TRUSTED_PROXIES` contains immediate proxy addresses that may supply
`X-Forwarded-For`. Never trust a network or address through which an untrusted
client can connect while choosing arbitrary forwarding headers.

### PostgreSQL

Set a PostgreSQL URL while retaining the data directory for attachments:

```ini
RACEBIN_DATABASE_URL=postgresql://racebin:replace-me@localhost/racebin
RACEBIN_DATA_DIR=/var/lib/racebin
```

Restrict `/etc/racebin.conf` to `0640` or tighter when it contains credentials.
Racebin applies the appropriate migrations at startup. See the
[database guide](database.md) for backups and SQLite-to-PostgreSQL migration.

## Create the first administrator

Create the initial account before exposing the service publicly. For the
generic service account, load the same environment used by systemd:

```bash
sudo -u racebin sh -c \
  'set -a; . /etc/racebin.conf; set +a; exec /usr/local/bin/racebin account create admin --admin'
```

For the Arch package, use `http` and `/usr/bin/racebin` instead. The command
prompts without echo for a password of at least 12 characters. Other account
and recovery commands are documented in the [account guide](accounts.md).

Enable and verify the service:

```bash
sudo systemctl enable --now racebin.service
systemctl status racebin.service
curl --fail http://127.0.0.1:7042/healthz
curl --fail http://127.0.0.1:7042/readyz
```

Both probes intentionally return an empty `204 No Content` response.
`healthz` confirms that the process is serving requests. `readyz` additionally
checks database availability. They are outside `/api/v1` so conventional
service monitors can request `/healthz` and `/readyz` directly.

## Reverse proxy

Bind Racebin to loopback and expose only the reverse proxy. The proxy should:

- terminate TLS;
- replace, rather than blindly preserve, client forwarding headers;
- allow request bodies at least as large as Racebin's configured attachment
  limit;
- forward requests without rewriting paths;
- permit streamed request and response bodies without imposing a shorter
  timeout than the intended uploads; and
- add HSTS only after the site is confirmed to work exclusively over HTTPS.

Racebin does not use WebSockets. It deliberately emits no CORS policy; its
built-in browser application is same-origin.

### Nginx

The certificate directives depend on the local ACME or certificate setup:

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name paste.example.com;

    ssl_certificate     /etc/ssl/paste.example.com/fullchain.pem;
    ssl_certificate_key /etc/ssl/paste.example.com/key.pem;

    client_max_body_size 4g;
    add_header Strict-Transport-Security "max-age=31536000" always;

    location / {
        proxy_pass http://127.0.0.1:7042;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_set_header X-Forwarded-Proto https;
        proxy_request_buffering off;
    }
}
```

Set `RACEBIN_TRUSTED_PROXIES=127.0.0.1` when Nginx connects from IPv4 loopback.
If Nginx connects over `::1`, trust that exact address as well.
`client_max_body_size` limits the complete request. Choose a proxy limit at
least slightly above Racebin's configured attachment limit to accommodate
multipart fields and framing, without accepting unnecessarily large requests.

### Caddy

Caddy obtains and renews certificates automatically for a public hostname:

```caddyfile
paste.example.com {
    request_body {
        max_size 4GB
    }

    header Strict-Transport-Security "max-age=31536000"
    reverse_proxy 127.0.0.1:7042
}
```

Caddy supplies the immediate client address in `X-Forwarded-For`; configure
`RACEBIN_TRUSTED_PROXIES=127.0.0.1`. Review Caddy's trusted-proxy configuration
separately if another proxy or CDN sits in front of it.

## Security checklist

- Expose ports 80/443 for the proxy, not Racebin's application port.
- Keep secure cookies enabled and use one canonical HTTPS origin.
- Keep `/etc/racebin.conf` and the data directory inaccessible to unrelated
  users.
- Treat the database, configuration, and attachment directory as one security
  boundary and one backup set.
- Configure only immediate, controlled proxy addresses as trusted.
- Create accounts through deliberate invitations and periodically review API
  keys and active sessions.
- Back up the installation before upgrades that may apply database migrations.

## Upgrades, logs, and backups

Build or install the new package, then restart Racebin:

```bash
sudo systemctl restart racebin.service
journalctl -u racebin.service -n 100 --no-pager
curl --fail http://127.0.0.1:7042/readyz
```

Racebin applies forward database migrations during startup. Take a backup first;
do not assume an older binary can read a schema after a migration.

For SQLite, stop Racebin and back up `database.sqlite` together with the entire
attachment tree. For PostgreSQL, coordinate a PostgreSQL backup with a backup
of the data directory. See the [database guide](database.md) for consistent
online-backup cautions, restore steps, and database-copy guidance.

The normal log destination under systemd is journald. Access logs contain the
client address, HTTP method, status, response size, user agent, and duration;
they deliberately omit request targets so invitation, reset, and final-read
tokens are not written to the access log.

Racebin performs expiration and orphan cleanup at startup and hourly in process,
so no separate cleanup service is required. It removes expired sessions and
password-reset tokens, stale authentication/idempotency/read records, expired
or consumed pastes after their grant window, old expired invitations, stale
upload staging files, and orphaned attachment directories. The supported
deployment is one Racebin process; multiple replicas would require shared
attachment storage and additional concurrency review.

Arch package upgrades may install `/etc/racebin.conf.pacnew` when the packaged
example changes. Compare and merge it deliberately; do not replace local
database URLs, public origins, or credentials blindly.

## Troubleshooting

### Login succeeds locally but not through the public site

Confirm the public site uses HTTPS, `RACEBIN_INSECURE_COOKIE` is unset, and the
browser is using the same origin as `RACEBIN_PUBLIC_URL`. Inspect the service
and proxy logs without recording credentials or request bodies.

### Generated links use the wrong origin or are relative

Set `RACEBIN_PUBLIC_URL` to the complete external HTTPS origin, restart the
service, and inspect `/api/v1/capabilities` for `web_base_url` and
`api_base_url`.

### Uploads fail with HTTP 413

Check both `RACEBIN_MAX_ATTACHMENT_SIZE_MB` and the proxy's request-body limit.
The proxy limit must allow multipart overhead in addition to file content.
Racebin's text/rich-text input limit is 2 MiB and is independent of the
attachment setting.

### Racebin rejects a request before the upload completes

Racebin allows 15 seconds to receive request headers and limits the server to
1,024 simultaneous connections. Check slow clients, proxy buffering, proxy
timeouts, and service load. Large attachment bodies are streamed once the
request has been accepted rather than buffered completely in application
memory.

### The service cannot open its database or attachments

Verify that the configured service account owns the data directory, that its
mode is `0700`, and that the database URL is available in the service
environment. With PostgreSQL, test connectivity using the same host and
credentials.

### Readiness fails

`/readyz` returns failure when the database is unavailable. Check
`journalctl -u racebin.service`, database availability, credentials, and local
filesystem permissions before restarting repeatedly.

### Client addresses are incorrect

Verify the proxy overwrites `X-Forwarded-For`, connects from an address listed
in `RACEBIN_TRUSTED_PROXIES`, and is not itself accepting spoofed forwarding
headers from an untrusted upstream.

## Related documentation

- [Accounts and recovery](accounts.md)
- [Database selection, backup, and migration](database.md)
- [HTTP API](api.md)
- [Architecture](architecture.md)
- [Development and testing](testing.md)
