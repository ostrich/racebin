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
      .catch(() => ({ error: { message: response.statusText } }));
    throw new ApiError(response.status, body.error?.message ?? response.statusText);
  }
  if (invalidateQueries) clearQueryCache();
  return response.status === 204
    ? (undefined as T)
    : (response.json() as Promise<T>);
}
