import { afterEach, describe, expect, it, vi } from "vitest";
import { apiKeyFromWire, invitationFromWire, pasteFromWire } from "./normalize";
import { ApiError, setSessionInvalidHandler, transport } from "./transport";
import { appState } from "../app/state";
import type { components } from "./generated";
import type { Paste } from "../types";

type WirePasteResource = components["schemas"]["PasteResource"];

describe("API wire mapping", () => {
  afterEach(() => {
    setSessionInvalidHandler();
    vi.unstubAllGlobals();
  });
  it("creates an internal paste without mutating or retaining duplicate wire fields", () => {
    const wire: WirePasteResource = {
      id: "example-paste",
      url: "/pastes/example-paste",
      api_url: "/api/v1/pastes/example-paste",
      read_url: "/api/v1/pastes/example-paste/reads",
      title: "Example",
      format: "text",
      language: "javascript",
      body: { format: "text", content: "const answer = 42;", language: "javascript" },
      visibility: "private",
      owner_id: 1,
      folder_id: null,
      created_at: "2023-11-14T22:13:20Z",
      updated_at: "2023-11-14T22:13:21Z",
      modified_at: null,
      expires_at: null,
      last_read_at: null,
      read_count: 0,
      read_limit: null,
      attachment_count: 0,
      size_bytes: 18,
      attachments: []
    };
    const before = structuredClone(wire);
    const paste = pasteFromWire(wire, '"paste-example-paste-1"') as Paste;

    expect(wire).toEqual(before);
    expect(paste.format).toBe("text");
    expect(paste.content).toBe("const answer = 42;");
    expect(paste._etag).toBe('"paste-example-paste-1"');
    expect(paste).not.toHaveProperty("body");
  });

  it("normalizes RFC 3339 timestamps in non-paste resources", () => {
    expect(
      apiKeyFromWire({
        id: 4,
        user_id: 1,
        owner_username: "owner",
        name: "Automation",
        token_prefix: "prefix",
        scopes: ["paste:read"],
        enabled: true,
        created_at: "2023-11-14T22:13:20Z",
        last_used_at: null
      })
    ).toMatchObject({ created_at: 1_700_000_000, last_used_at: null });
    expect(
      invitationFromWire({
        id: 7,
        token_prefix: "prefix",
        created_by_username: "owner",
        created_at: "2023-11-14T22:13:20Z",
        expires_at: "2027-01-15T08:00:00Z",
        status: "active"
      })
    ).toMatchObject({ created_at: 1_700_000_000, expires_at: 1_800_000_000 });
  });

  it("exposes mutation protocol headers to callers", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(null, {
          status: 204,
          headers: {
            ETag: '"paste-example-paste-2"',
            "Read-Token": "grant",
            "Idempotency-Replayed": "true"
          }
        })
      )
    );

    const result = await transport<void>("/pastes/example-paste", { method: "DELETE" });
    expect(result).toEqual({
      data: undefined,
      etag: '"paste-example-paste-2"',
      readToken: "grant",
      idempotencyReplayed: true
    });
  });

  it("preserves Problem Details identity and retry guidance", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            type: "urn:racebin:problem:validation_failed",
            title: "Unprocessable Entity",
            status: 422,
            detail: "Request is invalid"
          }),
          {
            status: 422,
            headers: { "Content-Type": "application/problem+json", "Retry-After": "3" }
          }
        )
      )
    );

    const error = (await transport("/pastes").catch((reason) => reason)) as ApiError;
    expect(error).toMatchObject({
      status: 422,
      problemType: "urn:racebin:problem:validation_failed",
      retryAfter: "3"
    });
  });

  it("reconciles an authenticated browser session only for session-ending problems", async () => {
    const ended = vi.fn();
    setSessionInvalidHandler(ended);
    appState.update((state) => ({
      ...state,
      session: {
        authenticated: true,
        user: { id: 1, username: "owner", role: "owner", password_change_required: false },
        csrf_token: "csrf",
        permissions: []
      }
    }));
    const response = (type: string) =>
      new Response(JSON.stringify({ type, title: "Unauthorized", status: 401, detail: "Denied" }), {
        status: 401,
        headers: { "Content-Type": "application/problem+json" }
      });
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(response("urn:racebin:problem:invalid_credentials"))
      .mockResolvedValueOnce(response("urn:racebin:problem:invalid_session"));
    vi.stubGlobal("fetch", fetchMock);

    await expect(transport("/session/reauthenticate", { method: "POST" })).rejects.toThrow();
    expect(ended).not.toHaveBeenCalled();
    await expect(transport("/pastes", { method: "GET" })).rejects.toThrow();
    expect(ended).toHaveBeenCalledOnce();
  });
});
