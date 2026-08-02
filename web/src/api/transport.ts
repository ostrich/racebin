import { currentState } from "../state";
import { clearQueryCache } from "../queryCache";

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

export type TransportOptions = {
  method?: "GET" | "POST" | "PATCH" | "DELETE";
  json?: unknown;
  body?: FormData;
  headers?: HeadersInit;
  invalidateQueries?: boolean;
};

export async function transport<T>(path: string, options: TransportOptions = {}): Promise<ApiResult<T>> {
  const method = options.method ?? "GET";
  const headers = new Headers(options.headers);
  headers.set("Accept", "application/json");
  let body: BodyInit | undefined = options.body;
  if (options.json !== undefined) {
    headers.set("Content-Type", "application/json");
    body = JSON.stringify(options.json);
  }
  const { session } = currentState();
  if (session.csrf_token && method !== "GET") headers.set("X-CSRF-Token", session.csrf_token);
  const response = await fetch(`/api/v1${path}`, {
    method,
    body,
    headers,
    credentials: "same-origin"
  });
  if (!response.ok) {
    const problem = await response.json().catch(() => ({ detail: response.statusText })) as {
      detail?: string;
      code?: string;
      errors?: Record<string, string[]>;
    };
    throw new ApiError(
      response.status,
      problem.detail ?? response.statusText,
      problem.code,
      problem.errors,
      response.headers.get("Retry-After") ?? undefined
    );
  }
  if (options.invalidateQueries ?? method !== "GET") clearQueryCache();
  return {
    data: response.status === 204 ? undefined as T : await response.json() as T,
    etag: response.headers.get("ETag"),
    readToken: response.headers.get("Read-Token"),
    idempotencyReplayed: response.headers.get("Idempotency-Replayed") === "true"
  };
}
