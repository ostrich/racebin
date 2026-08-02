import type { components } from "./generated";
import type { Attachment, Paste } from "../types";

type WireAttachment = components["schemas"]["AttachmentResource"];
type WirePasteResource = components["schemas"]["PasteResource"];
type WirePasteMetadata = components["schemas"]["PasteMetadataResource"];
type WirePasteSummary = components["schemas"]["PasteSummary"];
type WirePaste = WirePasteResource | WirePasteMetadata | WirePasteSummary;
type WireBody = components["schemas"]["BodyOutput"];

export function unixTimestamp(value: string | null | undefined): number | null {
  if (value === null || value === undefined) return null;
  const milliseconds = Date.parse(value);
  return Number.isFinite(milliseconds) ? Math.floor(milliseconds / 1000) : null;
}

function isWirePaste(value: unknown): value is WirePaste {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<WirePaste>;
  return typeof candidate.id === "string"
    && typeof candidate.url === "string"
    && (candidate.format === "text" || candidate.format === "rich_text")
    && typeof candidate.created_at === "string";
}

function attachmentFromWire(value: WireAttachment): Attachment {
  return { id: value.id, filename: value.filename, size_bytes: value.size_bytes, url: value.url };
}

function pasteFromWire(value: WirePaste, etag?: string | null): Paste {
  const resource = "attachments" in value ? value : undefined;
  const body = resource && "body" in resource ? resource.body as WireBody : undefined;
  return {
    id: value.id,
    url: value.url,
    api_url: resource?.api_url,
    read_url: resource?.read_url,
    source_url: resource?.source_url ?? undefined,
    archive_url: resource?.archive_url ?? undefined,
    _etag: etag ?? undefined,
    owner_id: value.owner_id ?? null,
    folder_id: value.folder_id ?? null,
    title: value.title,
    content: body?.format === "rich_text"
      ? body.plain_text
      : body?.content ?? ("excerpt" in value ? value.excerpt ?? "" : ""),
    document: body?.format === "rich_text" ? body.content : null,
    content_kind: value.format as "text" | "rich_text",
    language: body?.format === "text" ? body.language : value.language ?? "plaintext",
    visibility: value.visibility as "public" | "unlisted" | "private",
    created_at: unixTimestamp(value.created_at) ?? 0,
    updated_at: unixTimestamp(value.updated_at) ?? unixTimestamp(value.created_at) ?? 0,
    expires_at: unixTimestamp(value.expires_at),
    last_read_at: unixTimestamp(value.last_read_at),
    read_count: value.read_count,
    read_limit: value.read_limit ?? null,
    attachment_count: value.attachment_count,
    size_bytes: value.size_bytes,
    attachments: resource?.attachments.map(attachmentFromWire) ?? []
  };
}

export function normalizePayload(value: unknown, etag?: string | null): unknown {
  if (Array.isArray(value)) return value.map(item => normalizePayload(item));
  if (!value || typeof value !== "object") return value;
  if (isWirePaste(value)) return pasteFromWire(value, etag);
  const object = value as Record<string, unknown>;
  if (Array.isArray(object.items)) {
    const pagination = object.pagination && typeof object.pagination === "object"
      ? object.pagination as Record<string, unknown>
      : undefined;
    return {
      ...object,
      items: object.items.map(item => normalizePayload(item)),
      ...(pagination ? {
        page: pagination.page,
        page_size: pagination.page_size,
        total_items: pagination.total_items
      } : {})
    };
  }
  const normalized = Object.fromEntries(
    Object.entries(object).map(([key, item]) => [key, normalizePayload(item)])
  );
  for (const key of ["created_at", "last_login_at", "last_used_at", "expires_at"]) {
    const timestamp = normalized[key];
    if (typeof timestamp === "string") normalized[key] = unixTimestamp(timestamp);
  }
  return normalized;
}
