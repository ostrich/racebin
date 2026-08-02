import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { appState } from "../state";
import {
  createPaste, createPasteWithAttachments, deleteAttachment, readPaste, updatePaste
} from "./resources";

const pasteResponse = {
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

function jsonResponse(headers: HeadersInit = {}): Response {
  return new Response(JSON.stringify(pasteResponse), {
    status: 200,
    headers: { "Content-Type": "application/json", ...headers }
  });
}

describe("typed API resources", () => {
  beforeEach(() => {
    appState.update(state => ({
      ...state,
      session: {
        authenticated: true,
        user: { id: 1, username: "reader", role: "user", password_change_required: false },
        csrf_token: "csrf-example"
      }
    }));
  });
  afterEach(() => vi.unstubAllGlobals());

  it("serializes JSON and supplies CSRF and idempotency headers", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse());
    vi.stubGlobal("fetch", fetchMock);
    await createPaste({
      title: "Example",
      body: { format: "text", content: "hello", language: "plaintext" },
      visibility: "private"
    }, "create-key");

    const [, init] = fetchMock.mock.calls[0]!;
    const headers = new Headers(init.headers);
    expect(init.body).toBe(JSON.stringify({
      title: "Example",
      body: { format: "text", content: "hello", language: "plaintext" },
      visibility: "private"
    }));
    expect(headers.get("Content-Type")).toBe("application/json");
    expect(headers.get("X-CSRF-Token")).toBe("csrf-example");
    expect(headers.get("Idempotency-Key")).toBe("create-key");
  });

  it("constructs multipart paste requests within the API layer", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse());
    vi.stubGlobal("fetch", fetchMock);
    const file = new File(["hello"], "example.txt", { type: "text/plain" });
    await createPasteWithAttachments({
      title: "Files", format: "text", content: "body", language: "plaintext"
    }, [file], "multipart-key");

    const [, init] = fetchMock.mock.calls[0]!;
    expect(init.body).toBeInstanceOf(FormData);
    const body = init.body as FormData;
    expect(body.get("title")).toBe("Files");
    expect(body.get("file")).toBe(file);
    expect(new Headers(init.headers).has("Content-Type")).toBe(false);
  });

  it("propagates If-Match and replacement ETags", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ ETag: '"paste-example-paste-2"' }));
    vi.stubGlobal("fetch", fetchMock);
    const paste = await updatePaste("example-paste", { title: "Changed" }, '"paste-example-paste-1"');
    expect(new Headers(fetchMock.mock.calls[0]![1].headers).get("If-Match"))
      .toBe('"paste-example-paste-1"');
    expect(paste._etag).toBe('"paste-example-paste-2"');

    fetchMock.mockResolvedValueOnce(new Response(null, {
      status: 204, headers: { ETag: '"paste-example-paste-3"' }
    }));
    const deleted = await deleteAttachment("example-paste", 7, '"paste-example-paste-2"');
    expect(deleted.etag).toBe('"paste-example-paste-3"');
  });

  it("exposes final-read grants and replay state", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({
      "Read-Token": "download-grant",
      "Idempotency-Replayed": "true"
    }));
    vi.stubGlobal("fetch", fetchMock);
    const result = await readPaste("example-paste", "read-key");
    expect(result.paste.content).toBe("const answer = 42;");
    expect(result.readToken).toBe("download-grant");
    expect(result.idempotencyReplayed).toBe(true);
    expect(new Headers(fetchMock.mock.calls[0]![1].headers).get("Idempotency-Key")).toBe("read-key");
  });
});
