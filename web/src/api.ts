import { currentState } from "./state";
import { clearQueryCache } from "./queryCache";
import type {
  Attachment,
  Paste,
  PasteBody,
  WireAttachment,
  WirePasteBase,
  WirePasteResource,
  WirePasteSummary
} from "./types";

export type ApiResult<T> = {
  data: T;
  etag: string | null;
  readToken: string | null;
  idempotencyReplayed: boolean;
};

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public code?: string,
    public errors?: Record<string, string[]>,
    public retryAfter?: string
  ) {
    super(message);
  }
}

interface ApiRequestInit extends RequestInit {
  invalidateQueries?: boolean;
}

export async function requestApi<T>(path: string, init: ApiRequestInit = {}): Promise<T> {
  return (await requestApiResult<T>(path, init)).data;
}

export async function requestApiResult<T>(path: string, init: ApiRequestInit = {}): Promise<ApiResult<T>> {
  const {
    invalidateQueries = Boolean(init.method && init.method !== "GET"),
    ...requestInit
  } = init;
  const headers = new Headers(requestInit.headers);
  if (!headers.has("Accept")) headers.set("Accept", "application/json");
  if (requestInit.body && !(requestInit.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  const { session } = currentState();
  if (session.csrf_token && requestInit.method && requestInit.method !== "GET") {
    headers.set("X-CSRF-Token", session.csrf_token);
  }
  const response = await fetch(`/api/v1${path}`, { credentials: "same-origin", ...requestInit, headers });
  if (!response.ok) {
    const body = await response
      .json()
      .catch(() => ({ detail: response.statusText })) as {
        detail?: string; code?: string; errors?: Record<string, string[]>;
        error?: { message?: string };
      };
    throw new ApiError(
      response.status,
      body.detail ?? body.error?.message ?? response.statusText,
      body.code,
      body.errors,
      response.headers.get("Retry-After") ?? undefined
    );
  }
  if (invalidateQueries) clearQueryCache();
  const etag = response.headers.get("ETag");
  const value = response.status === 204 ? undefined : await response.json();
  return {
    data: normalizePayload(value, etag) as T,
    etag,
    readToken: response.headers.get("Read-Token"),
    idempotencyReplayed: response.headers.get("Idempotency-Replayed") === "true"
  };
}

function unixTimestamp(value: string | null | undefined): number | null {
  if (value === null || value === undefined) return null;
  const milliseconds = Date.parse(value);
  return Number.isFinite(milliseconds) ? Math.floor(milliseconds / 1000) : null;
}

function isWirePaste(value: unknown): value is WirePasteResource | WirePasteSummary {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<WirePasteBase>;
  return typeof candidate.id === "string"
    && typeof candidate.url === "string"
    && (candidate.format === "text" || candidate.format === "rich_text")
    && typeof candidate.created_at === "string";
}

function attachmentFromWire(value: WireAttachment): Attachment {
  return { id: value.id, filename: value.filename, size_bytes: value.size_bytes, url: value.url };
}

function pasteFromWire(
  value: WirePasteResource | WirePasteSummary,
  etag?: string | null
): Paste {
  const resource = "attachments" in value ? value : undefined;
  const body: PasteBody | undefined = resource?.body;
  return {
    id: value.id,
    url: value.url,
    api_url: resource?.api_url,
    read_url: resource?.read_url,
    source_url: resource?.source_url,
    archive_url: resource?.archive_url,
    _etag: etag ?? undefined,
    owner_id: value.owner_id ?? null,
    folder_id: value.folder_id ?? null,
    title: value.title,
    content: body?.format === "rich_text"
      ? body.plain_text
      : body?.content ?? ("excerpt" in value ? value.excerpt ?? "" : ""),
    document: body?.format === "rich_text" ? body.content : null,
    content_kind: value.format,
    language: body?.format === "text"
      ? body.language
      : value.language ?? "plaintext",
    visibility: value.visibility,
    created_at: unixTimestamp(value.created_at) ?? 0,
    updated_at: unixTimestamp(value.updated_at) ?? unixTimestamp(value.created_at) ?? 0,
    expires_at: unixTimestamp(value.expires_at),
    last_read_at: unixTimestamp(value.last_read_at),
    read_count: value.read_count,
    read_limit: value.read_limit,
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
