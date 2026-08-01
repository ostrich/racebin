import { currentState } from "./state";
import { clearQueryCache } from "./queryCache";

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
  }
}

interface ApiRequestInit extends RequestInit {
  invalidateQueries?: boolean;
}

export async function requestApi<T>(path: string, init: ApiRequestInit = {}): Promise<T> {
  const {
    invalidateQueries = Boolean(init.method && init.method !== "GET"),
    ...requestInit
  } = init;
  const headers = new Headers(requestInit.headers);
  if (requestInit.body && !(requestInit.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  const { session } = currentState();
  if (session.csrf_token && requestInit.method && requestInit.method !== "GET") {
    headers.set("X-CSRF-Token", session.csrf_token);
  }
  const response = await fetch(`/api/v1${path}`, { ...requestInit, headers });
  if (!response.ok) {
    const body = await response
      .json()
      .catch(() => ({ detail: response.statusText }));
    throw new ApiError(response.status, body.detail ?? body.error?.message ?? response.statusText);
  }
  if (invalidateQueries) clearQueryCache();
  if (response.status === 204) return undefined as T;
  const data = await response.json();
  return normalizePayload(data, response.headers.get("ETag")) as T;
}

function unixTimestamp(value: unknown): number | null {
  if (value === null || value === undefined) return null;
  if (typeof value === "number") return value;
  const milliseconds = Date.parse(String(value));
  return Number.isFinite(milliseconds) ? Math.floor(milliseconds / 1000) : null;
}

function normalizePayload(value: unknown, etag?: string | null): unknown {
  if (Array.isArray(value)) return value.map(item => normalizePayload(item));
  if (!value || typeof value !== "object") return value;
  const object = value as Record<string, unknown>;
  if (Array.isArray(object.items)) object.items = object.items.map(item => normalizePayload(item));
  if (object.pagination && typeof object.pagination === "object") {
    const pagination = object.pagination as Record<string, unknown>;
    object.page = pagination.page;
    object.page_size = pagination.page_size;
    object.total_items = pagination.total_items;
  }
  if (typeof object.url === "string" && typeof object.id === "string" && typeof object.format === "string") {
    const body = object.body as Record<string, unknown> | undefined;
    object.content_kind = object.format;
    object.content = body?.format === "rich_text"
      ? String(body.plain_text ?? "")
      : String(body?.content ?? object.excerpt ?? "");
    object.document = body?.format === "rich_text" ? body.content : null;
    object.language = body?.format === "text"
      ? String(body.language ?? object.language ?? "plaintext")
      : "plaintext";
    object.owner_id ??= null;
    object.folder_id ??= null;
    object.attachments ??= [];
    object.created_at = unixTimestamp(object.created_at) ?? 0;
    object.updated_at = unixTimestamp(object.updated_at) ?? object.created_at;
    object.expires_at = unixTimestamp(object.expires_at);
    object.last_read_at = unixTimestamp(object.last_read_at);
    if (etag) object._etag = etag;
  }
  return object;
}
