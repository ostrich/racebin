# HTTP API

Racebin exposes its supported API under `/api/v1`. The live service document is
available at `/api/v1`, and the generated OpenAPI document is at
`/api/v1/openapi.json`.

## Authentication

Create an API key from **Account → API keys**, then send it as a bearer token:

```sh
curl -H 'Authorization: Bearer YOUR_API_KEY' \
  https://example.com/api/v1/pastes?owner=me
```

Browser sessions use the session cookie and `X-CSRF-Token`. API clients should
use bearer authentication and do not send CSRF tokens. Creating a paste requires
authentication.

API keys have explicit scopes: `paste:read`, `paste:write`, `paste:delete`, and
`paste:list`. Administrative scopes are documented by the OpenAPI endpoint and
the Help page.

Errors use `application/problem+json` with `type`, `title`, `status`, and
`detail` fields.

## Discovery

- `GET /api/v1/capabilities` reports enabled formats, visibility modes, and
  upload limits.
- `GET /api/v1/languages` lists accepted syntax names and aliases.
- `GET /healthz` reports whether the process is running.
- `GET /readyz` reports whether the database is available.

## Create a paste

The canonical JSON representation uses a tagged `body`:

```sh
curl -X POST https://example.com/api/v1/pastes \
  -H 'Authorization: Bearer YOUR_API_KEY' \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: a-new-unique-value' \
  -d '{
    "title": "Example",
    "body": {
      "format": "text",
      "content": "const answer = 42;",
      "language": "javascript"
    },
    "visibility": "unlisted"
  }'
```

Rich text uses sanitized HTML on the wire:

```json
{"body":{"format":"rich_text","content":"<h1>Scene</h1><p>Text</p>"}}
```

Racebin also accepts `text/plain`, `text/markdown`, `text/html`, URL-encoded
forms, and multipart forms at the same endpoint. Raw uploads can put metadata in
the query string, which makes a generic uploader configuration straightforward:

```sh
curl -X POST \
  'https://example.com/api/v1/pastes?title=Example&visibility=unlisted&language=javascript' \
  -H 'Authorization: Bearer YOUR_API_KEY' \
  -H 'Content-Type: text/plain' \
  -H 'Accept: text/plain' \
  --data-binary @example.js
```

With `Accept: text/plain`, creation returns only the absolute paste URL. JSON is
the default. `Idempotency-Key` is optional but recommended for retried uploads;
reuse with different content returns `409 Conflict`.

Multipart creation is atomic from the caller's perspective and supports a body,
files, or both. Text fields use the JSON field names and every attachment uses a
repeated `file` part:

```sh
curl -X POST https://example.com/api/v1/pastes \
  -H 'Authorization: Bearer YOUR_API_KEY' \
  -F 'title=Build output' \
  -F 'visibility=private' \
  -F 'content=See attached logs.' \
  -F 'file=@build.log' \
  -F 'file=@report.txt'
```

`expires_at` is RFC 3339. `expires_in` is a positive number of seconds. They
cannot be combined.

## Read a paste

`GET /api/v1/pastes/{id}` returns metadata without incrementing the read count.
It deliberately does not consume a limited paste.

`POST /api/v1/pastes/{id}/reads` performs a read and returns content. Send an
`Idempotency-Key` when retrying. Owners and administrators receive a
`source_url`; retrieving that URL does not consume a read.

For a final permitted read, the response can include attachment and archive URLs
with a short-lived `read_token`. That capability remains valid for 15 minutes so
the reader can download the files after receiving the paste.

## Update and delete

Paste responses include an `ETag`. Mutating an existing paste requires
`If-Match`, preventing one client from silently overwriting another client's
changes:

```sh
curl -X PATCH https://example.com/api/v1/pastes/PASTE_ID \
  -H 'Authorization: Bearer YOUR_API_KEY' \
  -H 'If-Match: "paste-PASTE_ID-r3"' \
  -H 'Content-Type: application/json' \
  -d '{"title":"Revised title"}'
```

Use `DELETE /api/v1/pastes/{id}` with the same conditional header. `If-Match: *`
is available when the caller explicitly accepts overwriting the current
revision.

## Lists and folders

`GET /api/v1/pastes` returns `{items, pagination}`. Common query parameters are:

- `q`, `page`, and `page_size`
- `owner=me`, `visibility`, `format`, and `language`
- `folder_id`, `unfiled`, and `has_attachments`
- `created_after`, `created_before`, `expiration`, and `read_limit`
- `min_reads`, `max_reads`, `min_size_bytes`, and `max_size_bytes`
- `sort` and `direction`

Folder endpoints are `GET/POST /folders` and `PATCH/DELETE /folders/{id}`. Move
owned pastes as a collection operation:

```http
PATCH /api/v1/pastes
Content-Type: application/json

{"ids":["PASTE_ID"],"folder_id":12}
```

Use `null` as `folder_id` to move pastes out of a folder.

## Content conversion

`POST /api/v1/content-conversions` converts between plain text and sanitized
rich-text HTML:

```json
{
  "source":{"format":"text","content":"Scene heading\n\nDialogue"},
  "target_format":"rich_text"
}
```

The internal editor document is intentionally not part of the public API.
