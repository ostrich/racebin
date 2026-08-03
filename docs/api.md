# HTTP API

Racebin exposes its supported resource API under `/api/v1`. The discovery
document is available at `/api/v1`, and the generated OpenAPI 3.1 contract is
at `/api/v1/openapi.json`. The API contract version is independent of the
Racebin server release version.

## Authentication

Create an API key from **Account → API keys**, then send it as a bearer token:

```sh
curl -H 'Authorization: Bearer YOUR_API_KEY' \
  https://example.com/api/v1/pastes?owner=me
```

Browser sessions use the session cookie and `X-CSRF-Token`. API clients should
use bearer authentication and do not send CSRF tokens. Creating a paste requires
authentication.

The browser session cookie is HTTP-only, uses path `/` and `SameSite=Lax`, and
is marked `Secure` unless the explicitly unsafe development option is enabled.
Persistent sessions additionally receive a 30-day cookie lifetime. Native and
automation clients should prefer bearer keys rather than emulating browser
cookies.

API keys have explicit scopes. The complete, machine-readable scope requirement
for an ordinary owner API key is exposed as `x-racebin-scopes` in OpenAPI.
Ownership requirements and administrative alternatives are separate from that
list; operations with nontrivial alternatives expose
`x-racebin-authorization`. Discovery also returns the supported scope catalog.
The user-facing scopes are `paste:read`, `paste:write`, `paste:delete`,
`paste:list`, and `api_key:manage`; administrative keys can additionally use
`paste:manage`, `user:manage`, and `invitation:manage`.

Errors use `application/problem+json` with `type`, `title`, `status`, and
`detail` fields. The `type` is a stable Racebin URN suitable for programmatic
classification; clients should not branch on the human-readable title or
detail.

## Origins and browser routes

Racebin's built-in deployment model is same-origin: it does not emit CORS
headers. A browser application hosted on another origin therefore needs an
operator-controlled reverse proxy policy. Enabling credentialed cross-origin
requests is a deployment security decision and is intentionally not enabled by
Racebin itself. CORS does not restrict native, command-line, or server-side API
clients.

The OpenAPI document covers `/api/v1` resources only. Human-facing routes such
as `/pastes/{id}`, `/login`, and `/help` belong to the bundled web application
and are not stable API operations. The server uses an explicit SPA route
allowlist; unknown web and API routes return 404 rather than silently rendering
the application shell.

## Discovery

- `GET /api/v1/capabilities` reports the server and API versions,
  authentication methods, scopes, accepted formats and upload media types,
  visibility modes, upload limits, and other input limits. When
  `RACEBIN_PUBLIC_URL` is configured, it also reports canonical `web_base_url`
  and `api_base_url` values; otherwise those fields are omitted rather than
  inferred from untrusted request headers.
- `GET /api/v1/languages` lists accepted syntax names and aliases.
- `GET /healthz` reports whether the process is running.
- `GET /readyz` reports whether the database is available.

The same probes are represented inside the API contract as
`GET /api/v1/health` and `GET /api/v1/readiness`. All four return an empty
`204 No Content` response when successful.

The OpenAPI document describes every supported operation, parameter, request
media type, response status, response header, and named response schema. It is
the authoritative contract for generated clients. Browser-only navigation and
editor state are deliberately absent from it; the Svelte application consumes
the same resource API as any other client.

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

Racebin normalizes supported rich-text markup and removes active content,
unsafe URLs, and unsupported attributes before storage. Returned rich-text HTML
is the sanitized representation and is not guaranteed to be byte-for-byte
identical to the submitted HTML. This guarantee is also included in the
OpenAPI rich-text schemas.

Racebin also accepts `text/plain`, `text/markdown`, `text/html`, URL-encoded
forms, and multipart forms at the same endpoint. Creation query parameters are
accepted only with the three raw text media types; JSON, URL-encoded, and
multipart requests must carry all creation fields in their body. Raw uploads
use their request body as content and can put the remaining metadata in the
query string, which makes a generic uploader
configuration straightforward:

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

The raw media type determines the representation: `text/plain` creates plain
text and may use the `language` query parameter, `text/markdown` creates plain
text with the Markdown language, and `text/html` creates sanitized rich text
and does not accept a language. Raw requests therefore do not accept `content`
or `format` query parameters. A new paste must contain non-empty text/rich-text
content or at least one attachment. Create fields may be omitted but may not be
JSON `null`.

Multipart creation is atomic from the caller's perspective and requires at
least one file; a text or rich-text body is optional. Text fields use the JSON
field names and every attachment uses a repeated `file` part:

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

All timestamps returned by the public API use RFC 3339 strings, including
paste, folder, invitation, API-key, and administrative-account timestamps.
Unix timestamps remain an internal storage detail and are never exposed by the
supported wire contract.

## Read a paste

`GET /api/v1/pastes/{id}` returns metadata without incrementing the read count.
It deliberately does not consume a limited paste.

`POST /api/v1/pastes/{id}/reads` performs a read and returns content. Send an
`Idempotency-Key` when retrying. Owners and administrators receive a
`source_url`; retrieving that URL does not consume a read.

For a final permitted read, Racebin returns a short-lived attachment grant in
the `Read-Token` response header. JSON responses also include that grant in each
attachment URL and the archive URL. The header makes the grant available to
clients requesting `text/plain` or `text/html` as well. It remains valid for 15
minutes so the reader can download files after receiving the paste.

An idempotently replayed consuming read returns the original logical result
without incrementing the read count again. Reusing an idempotency key for a
different paste or request returns a conflict.

## Attachments, archives, and QR output

Add files to an existing owned paste with
`POST /api/v1/pastes/{id}/attachments`. The request is multipart, every file
uses the repeated `file` field, and `If-Match` is required:

```sh
curl -X POST https://example.com/api/v1/pastes/PASTE_ID/attachments \
  -H 'Authorization: Bearer YOUR_API_KEY' \
  -H 'If-Match: "paste-PASTE_ID-r1"' \
  -F 'file=@example.txt'
```

The response contains the created attachment metadata and a replacement
`ETag`. Attachment deletion also requires `If-Match` and returns the new ETag.
Downloads authorize against the parent paste; a final-read `read_token` grants
temporary access after the paste itself has been consumed.

`GET /api/v1/pastes/{id}/archive` streams the paste content and all attachments
as a ZIP archive; archive input is limited to 64 MiB.
`GET /api/v1/pastes/{id}/qr` returns a PNG QR code when QR output is enabled
and a canonical public URL is configured. Binary response bodies are streams,
not JSON byte arrays. Consult OpenAPI for the precise media types, response
headers, and authorization alternatives.

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

Attachment uploads and deletions return the paste's new `ETag`. Bulk folder
moves and folder deletion return `{pastes:[{id,etag}]}` with a replacement ETag
for every affected paste, so clients can continue with a conditional mutation
without first refetching each resource.

In paste updates, omitted fields remain unchanged. `folder_id: null` makes the
paste unfiled, `expires_at: null` removes expiration, and `read_limit: null`
makes reads unlimited. The `title`, `body`, and `visibility` fields cannot be
`null`; send a replacement value or omit them. Administrative user updates
likewise accept only non-null `enabled` and `role` values.

## Lists and folders

`GET /api/v1/pastes` returns `{items, pagination}`. Common query parameters are:

- `q`, `page`, and `page_size`
- `owner=me`, `visibility`, `format`, and `language`
- `folder_id`, `unfiled`, and `has_attachments`
- `created_after`, `created_before`, `expiration`, and `read_limit`
- `min_reads`, `max_reads`, `min_size_bytes`, and `max_size_bytes`
- `sort` and `direction`

Pages are one-based. `page` defaults to `1`; `page_size` defaults to `30` and
must be between `1` and `100`. Results default to `sort=created&direction=desc`.
Valid sort fields are `created`, `title`, `reads`, `expires`, and `size`, and
direction is `asc` or `desc`.

`q` performs a case-insensitive search over paste ID, title, content, language,
and attachment filename; administrative listings also search owner usernames.
`owner` currently accepts `me`. Folder filters require `owner=me`, and
`folder_id` cannot be combined with `unfiled=true`. Creation-time and numeric
range endpoints are inclusive. `expiration` accepts `never` or `scheduled`, and
`read_limit` accepts `unlimited` or `limited`. Minimum values cannot exceed
their corresponding maximum values.

Metadata reads return `PasteMetadataResource`, which deliberately has no body.
Creating, updating, consuming, or retrieving source content returns
`PasteResource`, where `body` is required. This distinction lets generated
clients represent an intentionally omitted body without treating it as an
arbitrary missing field.

Folder endpoints are `GET/POST /folders` and `PATCH/DELETE /folders/{id}`. Move
owned pastes as a collection operation:

```http
PATCH /api/v1/pastes
Content-Type: application/json

{"ids":["PASTE_ID"],"folder_id":12}
```

Use `null` as `folder_id` to move pastes out of a folder.

Administrative paste listing uses the same filters, pagination envelope, and
paste summary representation at `GET /api/v1/admin/pastes`. Its summaries add
owner information rather than exposing the server's internal storage model.

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
