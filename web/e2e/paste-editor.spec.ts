import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

test("untouched paste form navigates without a discard prompt", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await expect(page.getByRole("heading", { name: "New paste" })).toBeVisible();
  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page).toHaveURL(/\/pastes$/);
  await expect(page.getByRole("heading", { name: "My pastes" })).toBeVisible();
});

test("switching an empty paste to rich text does not create unsaved content", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("rich_text");
  await expect(page.locator(".rich-text-editor")).toBeVisible();

  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page).toHaveURL(/\/pastes$/);
  await expect(page.getByRole("heading", { name: "Discard unsaved changes?" })).toHaveCount(0);
});

test("expiration presets populate a stable, customizable date control", async ({ page }) => {
  await mockApi(page, true);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/pastes/new");

  const expiration = page.getByRole("combobox", { name: "Expiration" });
  const date = page.getByLabel("Date and time");
  const readLimit = page.getByLabel("Read limit");
  await expect(expiration).toHaveValue("never");
  await expect(date).toBeDisabled();
  await expect(date).toHaveValue("Not applicable");

  await expiration.selectOption("1w");
  await expect(date).toBeEnabled();
  await expect(date).not.toHaveValue("");
  const [dateBox, readLimitBox] = await Promise.all([date.boundingBox(), readLimit.boundingBox()]);
  expect(dateBox!.x + dateBox!.width).toBeLessThan(readLimitBox!.x);

  await date.fill("2030-01-02T03:04");
  await expect(expiration).toHaveValue("custom");
  await expect(date).toHaveValue("2030-01-02T03:04");

  await expiration.selectOption("never");
  await expect(date).toBeDisabled();
  await expect(date).toHaveValue("Not applicable");
  await expiration.selectOption("custom");
  await expect(date).toBeEnabled();
  await expect(date).toHaveValue("");
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

test("resizing the text editor grows the complete editor and is retained across modes", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const editor = page.locator(".content-editor");
  await editor.evaluate(element => { (element as HTMLElement).style.height = "620px"; });
  await expect.poll(() => page.locator(".content-editor").evaluate(element =>
    element.getBoundingClientRect().height
  )).toBe(620);
  const layers = await page.locator(".code-editor").evaluate(container => ({
    editor: container.getBoundingClientRect().height,
    textarea: container.querySelector("textarea")!.getBoundingClientRect().height,
    overlay: container.querySelector("pre")!.getBoundingClientRect().height,
    gutter: container.querySelector(".line-numbers")!.getBoundingClientRect().height
  }));
  expect(layers.editor).toBe(620);
  expect(layers.textarea).toBe(618);
  expect(layers.overlay).toBe(layers.textarea);
  expect(layers.gutter).toBe(layers.textarea);
  await expect(page.getByRole("textbox", { name: "Paste content" })).toHaveCSS("resize", "none");

  await page.locator(".form-grid select").first().selectOption("rich_text");
  await expect(page.locator(".rich-text-editor")).toBeVisible();
  await expect(page.locator(".content-editor")).toHaveCSS("height", "620px");
  await expect(page.locator(".content-editor")).toHaveCSS("resize", "vertical");

  await page.locator(".content-editor").evaluate(element => {
    (element as HTMLElement).style.height = "700px";
  });
  await expect(page.locator(".content-editor")).toHaveCSS("height", "700px");
  await page.locator(".form-grid select").first().selectOption("text");
  await expect(page.getByRole("textbox", { name: "Paste content" })).toBeVisible();
  await expect(page.locator(".content-editor")).toHaveCSS("height", "700px");
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

test("paste form labels share the same dark-mode color", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const colors = await page.locator(".form-grid").evaluate(form => {
    const color = (element: Element | null) => getComputedStyle(element!).color;
    return {
      type: color(form.querySelector("label > span")),
      language: color(form.querySelector(".language-field > label")),
      folder: color([...form.querySelectorAll("label > span")]
        .find(label => label.textContent === "Folder") ?? null),
      visibility: color([...form.querySelectorAll("label > span")]
        .find(label => label.textContent === "Visibility") ?? null)
    };
  });
  expect(new Set(Object.values(colors)).size).toBe(1);
});

test("rich-text formatting uses a single-row icon toolbar and confirms clearing", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.locator(".form-grid select").first().selectOption("rich_text");
  const toolbar = page.getByRole("toolbar", { name: "Rich-text formatting" });
  await expect(toolbar.getByRole("button")).toHaveCount(21);
  await expect(toolbar.getByRole("button", { name: "Paragraph" })).toHaveText("¶");
  await expect(toolbar.getByRole("button", { name: "Heading 1" })).toHaveText("H1");
  await expect(toolbar.getByRole("button", { name: "Bulleted list" }).locator("svg")).toBeVisible();
  const rows = await toolbar.getByRole("button").evaluateAll(buttons =>
    new Set(buttons.map(button => Math.round(button.getBoundingClientRect().top))).size
  );
  expect(rows).toBe(1);

  await page.getByLabel("Rich-text paste content").fill("Formatted text");
  page.once("dialog", dialog => {
    expect(dialog.message()).toBe("Clear all formatting from this rich-text paste?");
    void dialog.dismiss();
  });
  await toolbar.getByRole("button", { name: "Clear all formatting" }).click();
});

test("ordered rich-text lists can be submitted", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.locator(".form-grid select").first().selectOption("rich_text");
  await page.getByLabel("Rich-text paste content").fill("First item");
  await page.getByRole("button", { name: "Numbered list" }).click();

  const submitted = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/pastes") && request.method() === "POST"
  );
  await page.getByRole("button", { name: "Create paste" }).click();
  const body = (await submitted).postDataJSON();
  expect(body.body).toMatchObject({ format: "rich_text" });
  expect(body.body.content).toContain("<ol>");
  expect(body.body.content).toContain("<li><p>First item</p></li>");
  expect(body).not.toHaveProperty("expires_at");
  expect(body).not.toHaveProperty("read_limit");
  expect(body).not.toHaveProperty("folder_id");
});

test("pasted links are normalized to the supported document contract", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.locator(".form-grid select").first().selectOption("rich_text");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.focus();
  await editor.evaluate(element => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/plain", "Relative link and phone");
    clipboard.setData("text/html", '<p><a href="/help" target="_self" rel="external" class="button" onclick="alert(1)">Relative link</a> and <a href="tel:+15551212">phone</a></p>');
    element.dispatchEvent(new ClipboardEvent("paste", {
      bubbles: true, cancelable: true, clipboardData: clipboard
    }));
  });

  const submitted = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/pastes") && request.method() === "POST"
  );
  await page.getByRole("button", { name: "Create paste" }).click();
  const body = (await submitted).postDataJSON();
  expect(body.body).toMatchObject({ format: "rich_text" });
  expect(body.body.content).toContain('href="/help"');
  expect(body.body.content).toContain('rel="noopener noreferrer nofollow"');
  expect(body.body.content).not.toContain("onclick");
  expect(body.body.content).not.toContain("tel:");
});

test("rich-text conversion populates the plain-text editor", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const type = page.locator(".form-grid select").first();
  await type.selectOption("rich_text");
  await page.getByLabel("Rich-text paste content").fill("Rich content");
  await type.selectOption("text");
  await expect(page.getByRole("heading", { name: "Convert to text?" })).toBeVisible();
  await expect(page.locator(".conversion-dialog pre")).toContainText(paste.content);
  await page.getByRole("button", { name: "Convert" }).click();
  await expect(page.locator(".code-editor textarea")).toHaveValue(paste.content);
});

test("edit page shows current attachments", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/sample-paste/edit");
  await expect(page.getByText("Current attachments")).toBeVisible();
  await expect(page.getByRole("link", { name: /example.txt/ })).toBeVisible();
  await expect(page.getByText(/takes effect immediately/)).toBeVisible();
});

test("attachment deletion carries the returned revision into the next edit", async ({ page }) => {
  await mockApi(page, true);
  let deleteMatch = "";
  let patchMatch = "";
  await page.route("**/api/v1/pastes/sample-paste**", async route => {
    const request = route.request();
    const pathname = new URL(request.url()).pathname;
    if (pathname.endsWith("/source") && request.method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { ETag: '"paste-sample-paste-1"' },
        body: JSON.stringify(paste)
      });
    }
    if (pathname.endsWith("/attachments/7") && request.method() === "DELETE") {
      deleteMatch = request.headers()["if-match"] ?? "";
      return route.fulfill({ status: 204, headers: { ETag: '"paste-sample-paste-2"' } });
    }
    if (pathname === "/api/v1/pastes/sample-paste" && request.method() === "PATCH") {
      patchMatch = request.headers()["if-match"] ?? "";
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { ETag: '"paste-sample-paste-3"' },
        body: JSON.stringify({ ...paste, attachments: [], attachment_count: 0 })
      });
    }
    return route.fallback();
  });

  await page.goto("/pastes/sample-paste/edit");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "Delete example.txt" }).click();
  await expect(page.getByRole("link", { name: /example.txt/ })).toBeHidden();
  await page.getByLabel("Title").fill("Updated after attachment removal");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect.poll(() => [deleteMatch, patchMatch]).toEqual([
    '"paste-sample-paste-1"',
    '"paste-sample-paste-2"'
  ]);
});

test("failed edit attachment upload preserves the saved revision and retry state", async ({ page }) => {
  await mockApi(page, true);
  const patchHeaders: string[] = [];
  await page.route("**/api/v1/pastes/sample-paste**", async route => {
    const request = route.request();
    const pathname = new URL(request.url()).pathname;
    if (pathname === "/api/v1/pastes/sample-paste" && request.method() === "PATCH") {
      patchHeaders.push(request.headers()["if-match"] ?? "");
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { ETag: '"paste-sample-paste-2"' },
        body: JSON.stringify({ ...paste, title: "Saved title" })
      });
    }
    if (pathname.endsWith("/attachments") && request.method() === "POST") {
      return route.fulfill({
        status: 422,
        contentType: "application/problem+json",
        body: JSON.stringify({ detail: "Attachment was rejected" })
      });
    }
    return route.fallback();
  });
  await page.goto("/pastes/sample-paste/edit");
  await page.getByLabel("Title").fill("Saved title");
  await page.getByLabel("Add attachments").setInputFiles({
    name: "retry.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("retry")
  });
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Paste changes were saved, but attachments were not uploaded"
  );
  await expect(page).toHaveURL(/\/pastes\/sample-paste\/edit$/);
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect.poll(() => patchHeaders).toEqual(["*", "\"paste-sample-paste-2\""]);
});
