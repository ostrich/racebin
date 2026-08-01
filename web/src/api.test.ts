import { describe, expect, it } from "vitest";
import { normalizePayload } from "./api";
import type { Paste, WirePasteResource } from "./types";

describe("API wire mapping", () => {
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
});
