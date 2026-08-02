import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, normalizePayload, requestApi, requestApiResult } from "./api";
import type { Paste, WirePasteResource } from "./types";

describe("API wire mapping", () => {
  afterEach(() => vi.unstubAllGlobals());
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
      expires_at: null,
      last_read_at: null,
      read_count: 0,
      read_limit: null,
      attachment_count: 0,
      size_bytes: 18,
      attachments: []
    };
    const before = structuredClone(wire);
    const paste = normalizePayload(wire, "\"paste-example-paste-1\"") as Paste;

    expect(wire).toEqual(before);
    expect(paste.content_kind).toBe("text");
    expect(paste.content).toBe("const answer = 42;");
    expect(paste._etag).toBe("\"paste-example-paste-1\"");
    expect(paste).not.toHaveProperty("format");
    expect(paste).not.toHaveProperty("body");
  });

  it("normalizes RFC 3339 timestamps in non-paste resources", () => {
    const response = {
      key: {
        id: 4,
        created_at: "2023-11-14T22:13:20Z",
        last_used_at: null
      },
      invitations: [
        { id: 7, expires_at: "2027-01-15T08:00:00Z" }
      ]
    };

    expect(normalizePayload(response)).toEqual({
      key: { id: 4, created_at: 1_700_000_000, last_used_at: null },
      invitations: [{ id: 7, expires_at: 1_800_000_000 }]
    });
  });

  it("exposes mutation protocol headers to callers", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, {
      status: 204,
      headers: {
        ETag: "\"paste-example-paste-2\"",
        "Read-Token": "grant",
        "Idempotency-Replayed": "true"
      }
    })));

    const result = await requestApiResult<void>("/pastes/example-paste", { method: "DELETE" });
    expect(result).toEqual({
      data: undefined,
      etag: "\"paste-example-paste-2\"",
      readToken: "grant",
      idempotencyReplayed: true
    });
  });

  it("preserves structured API errors and retry guidance", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(JSON.stringify({
      code: "validation_failed",
      detail: "Request is invalid",
      errors: { title: ["Title is too long"] }
    }), {
      status: 422,
      headers: { "Content-Type": "application/problem+json", "Retry-After": "3" }
    })));

    const error = await requestApi("/pastes").catch(reason => reason) as ApiError;
    expect(error).toMatchObject({
      status: 422,
      code: "validation_failed",
      errors: { title: ["Title is too long"] },
      retryAfter: "3"
    });
  });
});
