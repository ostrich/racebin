import { expect, test, type Page, type Route } from "@playwright/test";

const config = {
  site_name: "Racebin",
  max_attachment_size_bytes: 20 * 1024 * 1024,
  attachments_enabled: true,
  qr_codes_enabled: false
};
const user = {
  id: 1,
  username: "test-admin",
  role: "admin",
  enabled: true,
  password_change_required: false
};
const paste = {
  id: "sample-paste",
  owner_id: 1,
  title: "JavaScript example",
  content: "const answer = 42;\nconsole.log(answer);",
  document: null,
  content_kind: "text",
  language: "javascript",
  visibility: "unlisted",
  created_at: 1_700_000_000,
  expires_at: null,
  last_read_at: null,
  read_count: 2,
  read_limit: null,
  attachment_count: 1,
  size_bytes: 1064,
  attachments: [{ id: 7, filename: "example.txt", size_bytes: 1024 }]
};

async function json(route: Route, value: unknown, status = 200): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(value)
  });
}

async function mockApi(page: Page, authenticated = false): Promise<void> {
  await page.route("**/api/v1/**", async route => {
    const url = new URL(route.request().url());
    if (url.pathname === "/api/v1/session") {
      return json(route, authenticated
        ? { authenticated: true, user, csrf_token: "csrf" }
        : { authenticated: false });
    }
    if (url.pathname === "/api/v1/config") return json(route, config);
    if (url.pathname.endsWith("/consume")) return json(route, paste);
    if (url.pathname === "/api/v1/pastes/sample-paste") return json(route, paste);
    if (url.pathname === "/api/v1/pastes/convert") {
      const body = route.request().postDataJSON() as { source_kind: string; content?: string };
      return json(route, body.source_kind === "text"
        ? {
            content: body.content ?? "",
            document: { type: "doc", content: [{ type: "paragraph", content: [] }] }
          }
        : { content: paste.content, document: null });
    }
    if (url.pathname === "/api/v1/account/api-keys") {
      return json(route, [{
        id: 4, user_id: 1, name: "Automation", token_prefix: "abcd",
        scopes: ["paste:read", "paste:write"], enabled: true,
        created_at: 1_700_000_000, last_used_at: null
      }]);
    }
    if (url.pathname === "/api/v1/admin/users") return json(route, [user]);
    if (url.pathname === "/api/v1/admin/invitations") return json(route, [{
      id: 3, token_prefix: "invite", expires_at: 1_800_000_000,
      status: "Redeemed", redeemed_by_username: "reader"
    }]);
    if (url.pathname === "/api/v1/admin/api-keys") return json(route, [{
      id: 4, user_id: 1, name: "Automation", token_prefix: "abcd",
      scopes: ["paste:read", "paste:write"], enabled: true,
      created_at: 1_700_000_000, last_used_at: null
    }]);
    if (url.pathname === "/api/v1/pastes") {
      if (route.request().method() === "POST") return json(route, paste, 201);
      return json(route, { items: [paste], page: 1, page_size: 50, total_items: 1 });
    }
    return json(route, {});
  });
}

test("renders the public homepage and paste viewer", async ({ page }) => {
  await mockApi(page);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Racebin" })).toBeVisible();
  await page.getByRole("link", { name: "JavaScript example" }).click();
  await expect(page).toHaveURL(/\/pastes\/sample-paste$/);
  await expect(page.locator(".line-numbers")).toHaveText("1\n2");
  await expect(page.locator("code.hljs")).toContainText("const answer");
  await expect(page.getByRole("link", { name: /example.txt/ })).toBeVisible();
});

test("untouched paste form navigates without a discard prompt", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await expect(page.getByRole("heading", { name: "New paste" })).toBeVisible();
  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page).toHaveURL(/\/pastes$/);
  await expect(page.getByRole("heading", { name: "My pastes" })).toBeVisible();
});

test("editing triggers the custom discard dialog and detects JavaScript", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const editor = page.getByRole("textbox", { name: "Paste content" });
  await editor.fill("function greet(name) { console.log(`hello ${name}`); }");
  await expect(page.getByRole("combobox", { name: /Language/ })).toHaveValue("javascript");
  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page.getByRole("heading", { name: "Discard unsaved changes?" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page).toHaveURL(/\/pastes\/new$/);
});

test("code editor caret and highlighted text retain the same scroll viewport", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const editor = page.getByRole("textbox", { name: "Paste content" });
  const content = Array.from(
    { length: 90 },
    (_, index) => `${String(index + 1).padStart(3, "0")} ${"long line ".repeat(20)}`
  ).join("\n");
  await editor.fill(content);
  await editor.evaluate(element => {
    element.scrollTop = element.scrollHeight;
    element.scrollLeft = element.scrollWidth;
    element.dispatchEvent(new Event("scroll"));
  });
  await expect.poll(() => page.locator(".code-editor").evaluate(container => {
    const textarea = container.querySelector("textarea")!;
    const overlay = container.querySelector("pre")!;
    const gutter = container.querySelector<HTMLElement>(".line-numbers")!;
    return {
      heightsMatch: overlay.clientHeight === textarea.clientHeight
        && gutter.clientHeight === textarea.clientHeight,
      verticalScrollMatches: Math.abs(overlay.scrollTop - textarea.scrollTop) < 1
        && Math.abs(gutter.scrollTop - textarea.scrollTop) < 1,
      horizontalScrollMatches: Math.abs(overlay.scrollLeft - textarea.scrollLeft) < 1
    };
  })).toEqual({
    heightsMatch: true,
    verticalScrollMatches: true,
    horizontalScrollMatches: true
  });
});

test("empty rich-text conversion skips preview and disables language", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const textEditorHeight = await page.locator(".content-editor").evaluate(
    element => element.getBoundingClientRect().height
  );
  const textControlsTop = await page.locator(".form-grid").evaluate(
    element => element.getBoundingClientRect().top
  );
  await page.locator(".form-grid select").first().selectOption("rich_text");
  await expect(page.getByRole("heading", { name: /Convert to/ })).toHaveCount(0);
  await expect(page.getByRole("combobox", { name: /Language/ })).toBeDisabled();
  await expect(page.locator(".rich-text-editor")).toBeVisible();
  const richTextEditorHeight = await page.locator(".content-editor").evaluate(
    element => element.getBoundingClientRect().height
  );
  const richTextControlsTop = await page.locator(".form-grid").evaluate(
    element => element.getBoundingClientRect().top
  );
  expect(richTextEditorHeight).toBe(textEditorHeight);
  expect(richTextControlsTop).toBe(textControlsTop);
});

test("edit page shows current attachments", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/sample-paste/edit");
  await expect(page.getByText("Current attachments")).toBeVisible();
  await expect(page.getByRole("link", { name: /example.txt/ })).toBeVisible();
  await expect(page.getByText(/takes effect immediately/)).toBeVisible();
});

test("paste filters remain URL-addressable", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes");
  await page.getByLabel("Search").fill("javascript");
  await page.getByRole("button", { name: "Apply" }).click();
  await expect(page).toHaveURL(/search=javascript/);
  await expect(page.getByRole("link", { name: /Search: javascript/ })).toBeVisible();
});

test("account and admin ownership data render as structured controls", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/account");
  await expect(page.getByText("Automation")).toBeVisible();
  await expect(page.getByText(/paste:read, paste:write/)).toBeVisible();
  await page.getByRole("link", { name: "Admin", exact: true }).click();
  await page.getByRole("button", { name: /Invitations/ }).click();
  await expect(page.getByText("Redeemed by reader")).toBeVisible();
  await page.getByRole("button", { name: /API keys/ }).click();
  await expect(page.getByText("Owner: test-admin")).toBeVisible();
  await expect(page.getByLabel("Privileges").getByText("paste:write")).toBeVisible();
});
