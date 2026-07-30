import { state } from "./state";

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
  }
}

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !(init.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  if (state.session.csrf_token && init.method && init.method !== "GET") {
    headers.set("X-CSRF-Token", state.session.csrf_token);
  }
  const response = await fetch(`/api/v2${path}`, { ...init, headers });
  if (!response.ok) {
    const body = await response
      .json()
      .catch(() => ({ error: { message: response.statusText } }));
    throw new ApiError(response.status, body.error?.message ?? response.statusText);
  }
  return response.status === 204
    ? (undefined as T)
    : (response.json() as Promise<T>);
}
