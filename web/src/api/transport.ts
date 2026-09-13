import { currentState } from "../app/state";
import { clearQueryCache } from "../app/queryCache";
import type { components } from "./generated";

type ProblemDetails = components["schemas"]["ProblemDetails"];

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
    public problemType?: string,
    public retryAfter?: string
  ) {
    super(message);
  }
}

let sessionInvalidHandler: (() => void) | undefined;

export function setSessionInvalidHandler(handler?: () => void): void {
  sessionInvalidHandler = handler;
}

export function isSessionInvalidError(error: unknown): error is ApiError {
  return (
    error instanceof ApiError &&
    error.status === 401 &&
    ["urn:racebin:problem:invalid_session", "urn:racebin:problem:authentication_required"].includes(
      error.problemType ?? ""
    )
  );
}

export type TransportOptions = {
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  json?: unknown;
  body?: FormData;
  headers?: HeadersInit;
  invalidateQueries?: boolean;
};

export async function transport<T>(
  path: string,
  options: TransportOptions = {}
): Promise<ApiResult<T>> {
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
    const problem = (await response.json().catch(() => null)) as ProblemDetails | null;
    const error = new ApiError(
      response.status,
      problem?.detail ?? response.statusText,
      problem?.type,
      response.headers.get("Retry-After") ?? undefined
    );
    if (currentState().session.user && isSessionInvalidError(error)) sessionInvalidHandler?.();
    throw error;
  }
  if (options.invalidateQueries ?? method !== "GET") clearQueryCache();
  return {
    data: response.status === 204 ? (undefined as T) : ((await response.json()) as T),
    etag: response.headers.get("ETag"),
    readToken: response.headers.get("Read-Token"),
    idempotencyReplayed: response.headers.get("Idempotency-Replayed") === "true"
  };
}
