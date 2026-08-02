import { expect, request as createRequest, test, type APIRequestContext } from "@playwright/test";

const credentials = {
  username: "test-admin",
  password: "correct horse battery staple"
};

async function login(client: APIRequestContext): Promise<string> {
  const response = await client.post("/api/v1/session", { data: credentials });
  expect(response.ok()).toBe(true);
  return (await response.json()).csrf_token;
}

async function createKey(client: APIRequestContext, csrf: string, scopes: string[]): Promise<string> {
  const response = await client.post("/api/v1/account/api-keys", {
    headers: { "X-CSRF-Token": csrf },
    data: { name: `real-stack-${crypto.randomUUID()}`, scopes }
  });
  expect(response.status()).toBe(201);
  return (await response.json()).token;
}

test("browser authentication enforces CSRF and bearer scopes", async ({ request }) => {
  const csrf = await login(request);

  const rejected = await request.post("/api/v1/account/api-keys", {
    data: { name: "missing-csrf", scopes: ["paste:read"] }
  });
  expect(rejected.status()).toBe(403);
  expect(await rejected.json()).toMatchObject({ status: 403 });

  const token = await createKey(request, csrf, ["paste:read", "paste:list"]);
  const bearer = await createRequest.newContext({
    baseURL: "http://127.0.0.1:4174",
    extraHTTPHeaders: { Authorization: `Bearer ${token}` }
  });
  try {
    expect((await bearer.get("/api/v1/pastes?owner=me")).ok()).toBe(true);
    const denied = await bearer.post("/api/v1/pastes", {
      headers: { "Idempotency-Key": crypto.randomUUID() },
      data: { body: { format: "text", content: "denied", language: "plaintext" } }
    });
    expect(denied.status()).toBe(403);
    expect(await denied.json()).toMatchObject({ status: 403 });
  } finally {
    await bearer.dispose();
  }
});

test("idempotent creation and conditional updates preserve protocol state", async ({ request }) => {
  const csrf = await login(request);
  const token = await createKey(request, csrf, ["paste:read", "paste:write", "paste:delete"]);
  const bearer = await createRequest.newContext({
    baseURL: "http://127.0.0.1:4174",
    extraHTTPHeaders: { Authorization: `Bearer ${token}` }
  });
  try {
    const idempotencyKey = crypto.randomUUID();
    const input = {
      title: "Protocol state",
      body: { format: "text", content: "initial", language: "plaintext" },
      visibility: "private"
    };
    const created = await bearer.post("/api/v1/pastes", {
      headers: { "Idempotency-Key": idempotencyKey }, data: input
    });
    expect(created.status()).toBe(201);
    const paste = await created.json();
    const etag = created.headers().etag;
    expect(etag).toMatch(/^".+"$/);

    const replay = await bearer.post("/api/v1/pastes", {
      headers: { "Idempotency-Key": idempotencyKey }, data: input
    });
    expect(replay.ok()).toBe(true);
    expect(replay.headers()["idempotency-replayed"]).toBe("true");
    expect((await replay.json()).id).toBe(paste.id);

    const stale = await bearer.patch(`/api/v1/pastes/${paste.id}`, {
      headers: { "If-Match": '"stale"' }, data: { title: "Changed" }
    });
    expect(stale.status()).toBe(412);

    const updated = await bearer.patch(`/api/v1/pastes/${paste.id}`, {
      headers: { "If-Match": etag }, data: { title: "Changed" }
    });
    expect(updated.ok()).toBe(true);
    expect(updated.headers().etag).not.toBe(etag);
    expect((await updated.json()).title).toBe("Changed");
  } finally {
    await bearer.dispose();
  }
});

test("multipart attachment creation supports final-read download grants", async ({ request }) => {
  const csrf = await login(request);
  const token = await createKey(request, csrf, ["paste:read", "paste:write"]);
  const bearer = await createRequest.newContext({
    baseURL: "http://127.0.0.1:4174",
    extraHTTPHeaders: { Authorization: `Bearer ${token}` }
  });
  const anonymous = await createRequest.newContext({ baseURL: "http://127.0.0.1:4174" });
  try {
    const created = await bearer.post("/api/v1/pastes", {
      headers: { "Idempotency-Key": crypto.randomUUID() },
      multipart: {
        title: "Final read attachment",
        format: "text",
        content: "download once",
        language: "plaintext",
        visibility: "unlisted",
        read_limit: "1",
        file: { name: "example.txt", mimeType: "text/plain", buffer: Buffer.from("attachment body") }
      }
    });
    expect(created.status()).toBe(201);
    const paste = await created.json();
    expect(paste.attachments).toHaveLength(1);

    const consumed = await anonymous.post(`/api/v1/pastes/${paste.id}/reads`, {
      headers: { "Idempotency-Key": crypto.randomUUID() }
    });
    expect(consumed.ok()).toBe(true);
    expect(consumed.headers()["read-token"]).toBeTruthy();
    const finalRead = await consumed.json();
    expect(finalRead.attachments[0].url).toContain("read_token=");

    const download = await anonymous.get(finalRead.attachments[0].url);
    expect(download.ok()).toBe(true);
    expect(await download.text()).toBe("attachment body");

    const exhausted = await anonymous.post(`/api/v1/pastes/${paste.id}/reads`, {
      headers: { "Idempotency-Key": crypto.randomUUID() }
    });
    expect(exhausted.status()).toBe(404);
  } finally {
    await bearer.dispose();
    await anonymous.dispose();
  }
});
