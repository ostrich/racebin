# Racebin API v2

The API is rooted at `/api/v2`. `/api/v1` is intentionally unavailable.

## Authentication

API clients send a key as a bearer token:

```http
Authorization: Bearer rbk_PREFIX_SECRET
```

Browser sessions use the `racebin_session` Secure, HttpOnly, SameSite=Lax
cookie. `GET /session` returns the CSRF token; session-authenticated mutations
must send it in `X-CSRF-Token`. Bearer-authenticated mutations do not use CSRF.

Keys are displayed only once and stored as SHA-256 digests. Available scopes:

| Scope | Permission |
| --- | --- |
| `paste:read` | Read owner-only pastes owned by the key's user |
| `paste:write` | Create and update that user's pastes and files |
| `paste:delete` | Delete owned pastes |
| `paste:list` | List owned pastes |
| `paste:admin` | Manage every paste |
| `user:admin` | Manage users |
| `invite:admin` | Manage invitations |
| `key:admin` | Manage keys and delegate held scopes |

A key with `key:admin` can grant only scopes it already has. Browser
administrators may grant any scope. Ordinary browser users may grant
`paste:read`, `paste:write`, `paste:delete`, and `paste:list`, but not
`paste:admin` or any other administrative scope. Disabled keys and keys owned
by disabled users do not authenticate.

## Pastes

Create a paste:

```bash
curl https://example.com/api/v2/pastes \
  -H "Authorization: Bearer $RACEBIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Example",
    "content": "console.log(\"hello\")",
    "kind": "text",
    "syntax": "js",
    "access": "unlisted",
    "expiration": null,
    "burn_after_reads": 0
  }'
```

`kind` is `text` or `url`. `access` is `public`, `unlisted`, or `owner`.
Expiration is a Unix timestamp in request bodies; `null` means no expiration.

List and filter:

```bash
curl 'https://example.com/api/v2/pastes?page=1&page_size=50&search=example&access=public'
curl -H "Authorization: Bearer $RACEBIN_KEY" \
  'https://example.com/api/v2/pastes?mine=true'
```

Lists return:

```json
{"items":[],"page":1,"page_size":50,"total":0}
```

Retrieve without consuming, consume a read, update, and delete:

```bash
curl https://example.com/api/v2/pastes/PASTE_ID
curl https://example.com/api/v2/pastes/PASTE_ID/consume
curl -X PATCH https://example.com/api/v2/pastes/PASTE_ID \
  -H "Authorization: Bearer $RACEBIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"title":"Updated","access":"public"}'
curl -X DELETE -H "Authorization: Bearer $RACEBIN_KEY" \
  https://example.com/api/v2/pastes/PASTE_ID
```

The ordinary paste endpoint is suitable for management and does not increment
the read count. `/pastes/{slug}/consume` and `/pastes/{slug}/raw` atomically
consume a read. File upload uses multipart POST to
`/pastes/{slug}/files`. Download or delete a file at
`/pastes/{slug}/files/{file_id}`. ZIP and QR output are at
`/pastes/{slug}/archive` and `/pastes/{slug}/qr`.

## Accounts And Administration

- `GET|POST|DELETE /session`
- `PATCH /account/password`
- `GET|POST /account/api-keys`
- `PATCH|DELETE /account/api-keys/{id}`
- `POST /invites/{token}/accept`
- `GET /admin/pastes`
- `GET /admin/users`, `PATCH /admin/users/{id}`
- `GET|POST /admin/invites`, `DELETE /admin/invites/{id}`
- `GET /admin/api-keys`, `PATCH|DELETE /admin/api-keys/{id}`

The live machine-readable route list is `/api/v2/openapi.json`.

## Errors

Errors have one envelope:

```json
{
  "error": {
    "code": "not_found",
    "message": "Paste not found",
    "details": null
  }
}
```
