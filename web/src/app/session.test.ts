import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import { getCapabilities, getLanguages, getSession, login, logout } from "../api";
import { appState, currentState } from "./state";
import {
  authenticationState,
  bootstrapApplication,
  refreshSession,
  replaceSession,
  signIn,
  signOut
} from "./session";
import { cachedQuery, loadQuery } from "./queryCache";
import type { Session } from "../types";

vi.mock("../api", () => ({
  getCapabilities: vi.fn(),
  getLanguages: vi.fn(),
  getSession: vi.fn(),
  login: vi.fn(),
  logout: vi.fn(),
  isSessionInvalidError: vi.fn(() => false)
}));

const anonymous: Session = { authenticated: false, permissions: [] };
const authenticated = {
  authenticated: true as const,
  user: { id: 1, username: "reader", role: "user" as const, password_change_required: false },
  csrf_token: "example-csrf",
  permissions: ["paste:read"]
};

describe("authentication lifecycle", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    replaceSession(anonymous);
    appState.update((state) => ({ ...state, ready: false }));
  });

  it("commits successful authentication and clears identity-specific cached data", async () => {
    await loadQuery("private-data", async () => "previous identity");
    vi.mocked(login).mockResolvedValue(authenticated);
    const pending = signIn({ username: "reader", password: "example password", remember: false });
    expect(get(authenticationState)).toEqual({ phase: "authenticating", previous: "anonymous" });
    await pending;
    expect(get(authenticationState).phase).toBe("authenticated");
    expect(currentState().session.user?.username).toBe("reader");
    expect(cachedQuery("private-data")).toBeUndefined();
  });

  it("restores the current stable phase when logout fails", async () => {
    replaceSession(authenticated);
    vi.mocked(logout).mockRejectedValue(new Error("offline"));
    await expect(signOut()).rejects.toThrow("offline");
    expect(get(authenticationState).phase).toBe("authenticated");
    expect(currentState().session.user?.id).toBe(1);
  });

  it("cannot resurrect a replaced session from a late refresh", async () => {
    replaceSession(authenticated);
    let resolve!: (session: Session) => void;
    vi.mocked(getSession).mockReturnValue(
      new Promise((accept) => {
        resolve = accept;
      })
    );
    const refresh = refreshSession();
    replaceSession(anonymous);
    resolve(authenticated);
    expect(await refresh).toBeNull();
    expect(get(authenticationState).phase).toBe("anonymous");
    expect(currentState().session.authenticated).toBe(false);
  });

  it("records bootstrap failure without publishing partial application state", async () => {
    vi.mocked(getSession).mockResolvedValue(authenticated);
    vi.mocked(getCapabilities).mockRejectedValue(new Error("offline"));
    vi.mocked(getLanguages).mockResolvedValue([]);
    await expect(bootstrapApplication()).rejects.toThrow("offline");
    expect(get(authenticationState).phase).toBe("failed");
    expect(currentState().ready).toBe(false);
    expect(currentState().session.authenticated).toBe(false);
  });
});
