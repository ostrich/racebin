import { expect, test } from "@playwright/test";
import routeManifest from "../../src/navigation/routes.json" with { type: "json" };

const spaRouteSamples = routeManifest.map((route) =>
  route.path.replace(":userId:int", "42").replace(":pasteId", "example").replace(":token", "token")
);

test("the compiled server serves every canonical browser route", async ({ request }) => {
  for (const path of spaRouteSamples) {
    const response = await request.get(path);
    expect(response.status(), path).toBe(200);
    expect(response.headers()["content-type"], path).toContain("text/html");
  }
});

test("compiled frontend creates and reads a paste through a disposable backend", async ({
  page,
  request
}) => {
  const capabilitiesResponse = await request.get("/api/v1/capabilities");
  expect(capabilitiesResponse.ok()).toBe(true);
  const capabilities = await capabilitiesResponse.json();
  expect(capabilities).toMatchObject({
    server_version: expect.any(String),
    api_version: "v1",
    web_base_url: "http://127.0.0.1:4174",
    api_base_url: "http://127.0.0.1:4174/api/v1",
    attachment_upload_media_types: ["multipart/form-data"]
  });
  expect(capabilities.paste_create_media_types).toEqual(
    expect.arrayContaining(["application/json", "text/plain", "multipart/form-data"])
  );

  await page.goto("/login");
  await page.getByLabel("Username").fill("test-admin");
  await page.getByLabel("Password").fill("correct horse battery staple");
  await page.getByRole("button", { name: "Log in" }).click();
  await expect(page).toHaveURL(/\/pastes$/);

  await page.getByRole("link", { name: "New paste", exact: true }).click();
  await page.getByLabel("Title").fill("Disposable stack smoke test");
  await page.getByLabel("Paste content").fill("const verified = true;");
  await page.getByRole("button", { name: "Create paste" }).click();

  await expect(page).toHaveURL(/\/pastes\/[^/]+$/);
  await expect(page.getByRole("heading", { name: "Disposable stack smoke test" })).toBeVisible();
  await expect(page.locator("code.hljs")).toContainText("const verified = true;");
});

test("rich-text structures survive visual editing, persistence, and server rendering", async ({
  page
}) => {
  await page.goto("/login");
  await page.getByLabel("Username").fill("test-admin");
  await page.getByLabel("Password").fill("correct horse battery staple");
  await page.getByRole("button", { name: "Log in" }).click();
  await expect(page).toHaveURL(/\/pastes$/);

  await page.goto("/pastes/new");
  await page.getByLabel("Title").fill("Rich-text round-trip test");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await expect(page.locator('.rich-text-editor[data-editor-ready="true"]')).toBeVisible();
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  const markdown = [
    "3. Third",
    "4. Fourth",
    "",
    "- [x] Complete",
    "- [ ] Pending",
    "",
    "| Line |",
    "| --- |",
    "| first<br>second |"
  ].join("\n");
  await source.fill(markdown);
  await page.getByRole("button", { name: "Visual", exact: true }).click();
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  await expect(source).toHaveValue(markdown);
  await page.getByRole("button", { name: "Create paste" }).click();

  await expect(page).toHaveURL(/\/pastes\/[^/]+$/);
  await expect(page.locator(".rich-text-viewer ol")).toHaveAttribute("start", "3");
  await expect(page.locator(".rich-text-viewer input[type=checkbox]")).toHaveCount(2);
  await expect(page.locator(".rich-text-viewer td br")).toHaveCount(1);
  const pasteId = new URL(page.url()).pathname.split("/").at(-1)!;
  const stored = await page.request.get(`/api/v1/pastes/${pasteId}/source`);
  expect(stored.ok()).toBe(true);
  expect((await stored.json()).body.content).toBe(markdown);
});
