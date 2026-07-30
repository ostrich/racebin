# Racebin API v2

The API is rooted at `/api/v2`. This release intentionally resets the v2
vocabulary; clients written for an earlier Racebin schema must be updated.

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
| `paste:read` | Read private pastes owned by the key's user |
| `paste:write` | Create and update that user's pastes and attachments |
| `paste:delete` | Delete owned pastes |
| `paste:list` | List owned pastes |
| `paste:manage` | Manage every paste |
| `user:manage` | Manage users |
| `invitation:manage` | Manage invitations |
| `api_key:manage` | Manage keys and delegate held scopes |

An API key with `api_key:manage` can grant only scopes it already holds.
Browser administrators may grant any scope. Ordinary users may grant the four
non-management paste scopes.

## Pastes

Create a paste:

```bash
curl https://example.com/api/v2/pastes \
  -H "Authorization: Bearer $RACEBIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Example",
    "content": "console.log(\"hello\")",
    "content_kind": "text",
    "language": "javascript",
    "visibility": "unlisted",
    "expires_at": null,
    "read_limit": null
  }'
```

`content_kind` is `text`, `rich_text`, or `redirect`. `visibility` is `public`, `unlisted`,
or `private`. Timestamp fields are Unix seconds. A null `expires_at` never
expires, and a null `read_limit` permits unlimited reads.

Rich-text pastes include a validated ProseMirror `document` object. Their
`content` field is server-generated plaintext used for search, previews, raw
downloads, copying, and clients that do not render rich text. Supported nodes
are paragraphs, headings 1–3, lists, blockquotes, horizontal rules, hard
breaks, text, and code blocks. Supported marks are bold, italic, underline,
strike, inline code, and safe HTTP(S) or mail links.

Convert between text and rich text without saving a paste:

```bash
curl -X POST https://example.com/api/v2/pastes/convert \
  -H "Content-Type: application/json" \
  -H "X-CSRF-Token: $RACEBIN_CSRF" \
  -d '{"source_kind":"text","target_kind":"rich_text","content":"SCENE 1\n\nADA\nHello.","document":null}'
```

The reverse conversion accepts the rich-text `document` and returns normalized
plaintext. Conversion intentionally excludes redirects.

List and filter:

```bash
curl 'https://example.com/api/v2/pastes?page=1&page_size=50&search=example&visibility=public'
curl -H "Authorization: Bearer $RACEBIN_KEY" \
  'https://example.com/api/v2/pastes?mine=true'
```

Lists return:

```json
{"items":[],"page":1,"page_size":50,"total_items":0}
```

Retrieve without consuming, consume a read, update, and delete:

```bash
curl https://example.com/api/v2/pastes/PASTE_ID
curl https://example.com/api/v2/pastes/PASTE_ID/consume
curl -X PATCH https://example.com/api/v2/pastes/PASTE_ID \
  -H "Authorization: Bearer $RACEBIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{"title":"Updated","visibility":"public"}'
curl -X DELETE -H "Authorization: Bearer $RACEBIN_KEY" \
  https://example.com/api/v2/pastes/PASTE_ID
```

`/pastes/{paste_id}/consume` and `/pastes/{paste_id}/raw` atomically consume a
read. Upload attachments with multipart POST to
`/pastes/{paste_id}/attachments`. Download or delete an attachment at
`/pastes/{paste_id}/attachments/{attachment_id}`. ZIP and QR output are at
`/pastes/{paste_id}/archive` and `/pastes/{paste_id}/qr`.

## Accounts And Administration

- `GET|POST|DELETE /session`
- `PATCH /account/password`
- `GET|POST /account/api-keys`
- `PATCH|DELETE /account/api-keys/{id}`
- `POST /invitations/{token}/redeem`
- `GET /admin/pastes`
- `GET /admin/users`, `PATCH /admin/users/{id}`
- `GET|POST /admin/invitations`, `DELETE /admin/invitations/{id}`
- `GET /admin/api-keys`, `PATCH|DELETE /admin/api-keys/{id}`

The machine-readable route list is `/api/v2/openapi.json`.

## Errors

```json
{"error":{"code":"not_found","message":"Paste not found","details":null}}
```
