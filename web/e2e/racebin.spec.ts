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
  folder_id: null,
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
const folderOverview = {
  items: [
    { id: 5, name: "Scripts", created_at: 1_700_000_000, paste_count: 1 },
    { id: 7, name: "sample-folder", created_at: 1_700_000_000, paste_count: 18 }
  ],
  total_count: 19,
  unfiled_count: 0
};

async function json(route: Route, value: unknown, status = 200): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(value)
  });
}

async function mockApi(
  page: Page,
  authenticated = false,
  options: {
    items?: Array<typeof paste>;
    delay?: number;
    viewPaste?: typeof paste;
    pastePage?: (url: URL) => { items: Array<typeof paste>; delay?: number };
    adminPastePage?: (url: URL) => { items: Array<typeof paste>; delay?: number };
  } = {}
): Promise<void> {
  const viewPaste = options.viewPaste ?? paste;
  let folders = structuredClone(folderOverview);
  await page.route("**/api/v1/**", async route => {
    const url = new URL(route.request().url());
    if (url.pathname === "/api/v1/session") {
      return json(route, authenticated
        ? { authenticated: true, user, csrf_token: "csrf" }
        : { authenticated: false });
    }
    if (url.pathname === "/api/v1/config") return json(route, config);
    if (url.pathname === "/api/v1/folders") {
      if (route.request().method() === "POST") {
        const body = route.request().postDataJSON() as { name: string };
        const folder = { id: 6, name: body.name, created_at: 1_700_000_001, paste_count: 0 };
        folders.items.push(folder);
        return json(route, folder, 201);
      }
      return json(route, folders);
    }
    if (url.pathname === "/api/v1/folders/5") {
      if (route.request().method() === "PATCH") {
        const body = route.request().postDataJSON() as { name: string };
        folders.items = folders.items.map(folder =>
          folder.id === 5 ? { ...folder, name: body.name } : folder);
        return json(route, folders.items[0]);
      }
      if (route.request().method() === "DELETE") {
        folders.items = folders.items.filter(folder => folder.id !== 5);
        return route.fulfill({ status: 204 });
      }
    }
    if (url.pathname === "/api/v1/pastes/folder") return route.fulfill({ status: 204 });
    if (url.pathname.endsWith("/consume")) return json(route, viewPaste);
    if (url.pathname === "/api/v1/pastes/sample-paste") return json(route, viewPaste);
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
    if (url.pathname === "/api/v1/admin/pastes") {
      const response = options.adminPastePage?.(url) ?? {
        items: options.items ?? [paste],
        delay: options.delay
      };
      if (response.delay) await new Promise(resolve => setTimeout(resolve, response.delay));
      return json(route, {
        items: response.items,
        page: Number(url.searchParams.get("page") ?? 1),
        page_size: 100,
        total_items: response.items.length
      });
    }
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
      const response = options.pastePage?.(url) ?? {
        items: options.items ?? [paste],
        delay: options.delay
      };
      if (response.delay) await new Promise(resolve => setTimeout(resolve, response.delay));
      return json(route, {
        items: response.items,
        page: Number(url.searchParams.get("page") ?? 1),
        page_size: 50,
        total_items: response.items.length
      });
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
  await expect(page.getByRole("checkbox", { name: "Wrap" })).toHaveCount(0);
});

test("wide paste offers synchronized sticky scrolling and aligned wrapped lines", async ({ page }) => {
  const content = Array.from(
    { length: 60 },
    (_, index) => `${String(index + 1).padStart(3, "0")} ${"wide content ".repeat(24)}`
  ).join("\n");
  await mockApi(page, false, { viewPaste: { ...paste, content } });
  await page.goto("/pastes/sample-paste");
  const code = page.locator(".paste-code-content-scroll");
  await expect.poll(() => code.evaluate(
    element => element.scrollWidth > element.clientWidth
  )).toBe(true);
  await page.evaluate(() => window.scrollTo(0, 500));
  const floating = page.getByRole("region", { name: "Horizontal paste scrollbar" });
  await expect(floating).toHaveClass(/visible/);
  const scrollbarAlignment = await page.locator(".paste-code-shell").evaluate(shell => {
    const gutteredViewer = shell.querySelector(".paste-code")!.getBoundingClientRect();
    const content = shell.querySelector(".paste-code-content-scroll")!.getBoundingClientRect();
    const stickyScrollbar = shell.querySelector(".paste-floating-scrollbar")!.getBoundingClientRect();
    return {
      startsAfterGutter: content.left > gutteredViewer.left,
      alignedWithContent: Math.abs(stickyScrollbar.left - content.left) < 1
    };
  });
  expect(scrollbarAlignment).toEqual({
    startsAfterGutter: true,
    alignedWithContent: true
  });
  const wrapTogglePosition = await page.getByRole("checkbox", { name: "Wrap" }).evaluate(input => {
    const toggle = input.closest("label")!.getBoundingClientRect();
    const heading = document.querySelector(".paste-view .page-heading")!.getBoundingClientRect();
    const viewer = document.querySelector(".paste-code-shell")!.getBoundingClientRect();
    return toggle.top >= heading.bottom && toggle.bottom <= viewer.top;
  });
  expect(wrapTogglePosition).toBe(true);
  await floating.evaluate(element => {
    element.scrollLeft = 240;
    element.dispatchEvent(new Event("scroll"));
  });
  await expect.poll(() => code.evaluate(element => element.scrollLeft)).toBeGreaterThan(200);

  const unwrappedFirstNumber = await page.locator(".line-numbers").evaluate(gutter => {
    const range = document.createRange();
    range.setStart(gutter.firstChild!, 0);
    range.setEnd(gutter.firstChild!, 1);
    const bounds = range.getBoundingClientRect();
    return { left: bounds.left + window.scrollX, top: bounds.top + window.scrollY };
  });
  await page.getByRole("checkbox", { name: "Wrap" }).check();
  await expect(page.locator(".paste-floating-scrollbar")).not.toHaveClass(/visible/);
  await expect.poll(() => code.evaluate(
    element => element.scrollWidth <= element.clientWidth + 1
  )).toBe(true);
  const lineLayout = await page.locator(".line-numbers.wrapped").evaluate(gutter => {
    const numbers = [...gutter.querySelectorAll<HTMLElement>("span")];
    const content = document.querySelector<HTMLElement>(".paste-code .content")!;
    const firstNumberRange = document.createRange();
    firstNumberRange.selectNodeContents(numbers[0]);
    const firstNumberBounds = firstNumberRange.getBoundingClientRect();
    return {
      count: numbers.length,
      firstGap: numbers[1].offsetTop - numbers[0].offsetTop,
      firstNumber: {
        left: firstNumberBounds.left + window.scrollX,
        top: firstNumberBounds.top + window.scrollY
      },
      firstLineAligned: Math.abs(
        numbers[0].getBoundingClientRect().top -
        (content.getBoundingClientRect().top + Number.parseFloat(getComputedStyle(content).paddingTop))
      ) < 1
    };
  });
  expect(lineLayout.count).toBe(60);
  expect(lineLayout.firstGap).toBeGreaterThan(22);
  expect(lineLayout.firstLineAligned).toBe(true);
  expect(Math.abs(lineLayout.firstNumber.left - unwrappedFirstNumber.left)).toBeLessThan(1);
  expect(Math.abs(lineLayout.firstNumber.top - unwrappedFirstNumber.top)).toBeLessThan(1);
});

test("untouched paste form navigates without a discard prompt", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await expect(page.getByRole("heading", { name: "New paste" })).toBeVisible();
  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page).toHaveURL(/\/pastes$/);
  await expect(page.getByRole("heading", { name: "My pastes" })).toBeVisible();
});

test("back and forward navigation restore list scroll positions after loading", async ({ page }) => {
  const items = Array.from({ length: 50 }, (_, index) => ({
    ...paste,
    id: `sample-paste-${index}`,
    title: `Paste ${String(index + 1).padStart(2, "0")}`
  }));
  await mockApi(page, true, { items, delay: 150 });
  await page.goto("/pastes");
  const link = page.getByRole("link", { name: "Paste 30" });
  await link.scrollIntoViewIfNeeded();
  const listScroll = await page.evaluate(() => window.scrollY);
  expect(listScroll).toBeGreaterThan(0);
  await link.click();
  await expect(page).toHaveURL(/\/pastes\/sample-paste-29$/);
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
  await page.goBack();
  await expect(page).toHaveURL(/\/pastes$/);
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(listScroll);
  await page.goForward();
  await expect(page).toHaveURL(/\/pastes\/sample-paste-29$/);
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
});

test("returning from a paste renders the cached list while it revalidates", async ({ page }) => {
  let listRequests = 0;
  await mockApi(page, true, {
    pastePage: () => {
      listRequests += 1;
      return { items: [paste], delay: listRequests > 1 ? 250 : 0 };
    }
  });
  await page.goto("/pastes");
  await page.getByRole("link", { name: "JavaScript example" }).click();
  await expect(page).toHaveURL(/\/pastes\/sample-paste$/);
  await page.getByRole("link", { name: "My pastes" }).click();
  await page.waitForTimeout(50);
  await expect(page).toHaveURL(/\/pastes$/);
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  await expect(page.getByText("Loading pastes…")).toHaveCount(0);
  expect(listRequests).toBe(2);
  await expect(page.locator(".paste-workspace")).toHaveAttribute("aria-busy", "false");
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

test("paste filters remain URL-addressable", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes");
  await page.getByLabel("Search").fill("javascript");
  await page.getByRole("button", { name: "Apply" }).click();
  await expect(page).toHaveURL(/search=javascript/);
  await expect(page.getByRole("link", { name: /Search: javascript/ })).toBeVisible();
});

test("query navigation retains list pages until their replacement is ready", async ({ page }) => {
  const filteredPaste = { ...paste, id: "filtered-paste", title: "Filtered result" };
  await mockApi(page, true, {
    pastePage: url => url.searchParams.has("search")
      ? { items: [filteredPaste], delay: 150 }
      : { items: [paste] },
    adminPastePage: url => url.searchParams.has("search")
      ? { items: [filteredPaste], delay: 150 }
      : { items: [paste] }
  });

  await page.goto("/pastes");
  await page.evaluate(() => {
    Object.assign(window, { __retainedList: document.querySelector(".paste-workspace") });
  });
  await page.getByLabel("Search").fill("filtered");
  await page.getByRole("button", { name: "Apply" }).click();
  await page.waitForTimeout(50);
  await expect(page.getByRole("complementary", { name: "Paste folders" })).toBeVisible();
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  expect(await page.evaluate(() =>
    (window as Window & { __retainedList: Element }).__retainedList
      === document.querySelector(".paste-workspace")
  )).toBe(true);
  await expect(page.getByRole("link", { name: "Filtered result" })).toBeVisible();

  await page.goto("/admin/pastes");
  await page.evaluate(() => {
    Object.assign(window, { __retainedAdmin: document.querySelector("main > section") });
  });
  await page.getByLabel("Search").fill("filtered");
  await page.getByRole("button", { name: "Apply" }).click();
  await page.waitForTimeout(50);
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  expect(await page.evaluate(() =>
    (window as Window & { __retainedAdmin: Element }).__retainedAdmin
      === document.querySelector("main > section")
  )).toBe(true);
  await expect(page.getByRole("link", { name: "Filtered result" })).toBeVisible();
});

test("the newest query response wins when list requests overlap", async ({ page }) => {
  const slowPaste = { ...paste, id: "slow-paste", title: "Slow result" };
  const fastPaste = { ...paste, id: "fast-paste", title: "Fast result" };
  await mockApi(page, true, {
    pastePage: url => {
      if (url.searchParams.get("folder_id") === "5") return { items: [slowPaste], delay: 250 };
      if (url.searchParams.get("unfiled") === "true") return { items: [fastPaste], delay: 25 };
      return { items: [paste] };
    }
  });
  await page.goto("/pastes");
  await page.getByRole("link", { name: /Scripts/ }).click();
  await page.waitForTimeout(20);
  await page.getByRole("link", { name: /Uncategorized/ }).click();
  await expect(page.getByRole("link", { name: "Fast result" })).toBeVisible();
  await page.waitForTimeout(300);
  await expect(page.getByRole("link", { name: "Fast result" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Slow result" })).toHaveCount(0);
});

test("list geometry remains stable when the document starts or stops overflowing", async ({ page }) => {
  const manyPastes = Array.from({ length: 40 }, (_, index) => ({
    ...paste,
    id: `overflow-paste-${index}`,
    title: `Overflow paste ${index}`
  }));
  await mockApi(page, true, {
    pastePage: url => url.searchParams.get("folder_id") === "5"
      ? { items: manyPastes }
      : { items: [] }
  });
  await page.goto("/pastes");
  const before = await page.locator(".paste-workspace-main").evaluate(element => {
    const bounds = element.getBoundingClientRect();
    return { left: bounds.left, right: bounds.right };
  });
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).scrollbarGutter))
    .toContain("stable");
  await page.getByRole("link", { name: /Scripts/ }).click();
  await expect(page.getByRole("link", { name: "Overflow paste 39" })).toBeVisible();
  const after = await page.locator(".paste-workspace-main").evaluate(element => {
    const bounds = element.getBoundingClientRect();
    return { left: bounds.left, right: bounds.right };
  });
  expect(after).toEqual(before);
});

test("folders filter the workspace and carry into new pastes", async ({ page }) => {
  await mockApi(page, true, { items: [{ ...paste, folder_id: 5 }] });
  await page.goto("/pastes?folder_id=5");
  await expect(page.getByRole("heading", { name: "Scripts" })).toBeVisible();
  await expect(page.getByRole("complementary", { name: "Paste folders" })
    .getByRole("link", { name: /Scripts/ })).toHaveClass(/current/);
  const selectedPaste = page.getByRole("checkbox", { name: /Select JavaScript example/ });
  await selectedPaste.check();
  const moveRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/pastes/folder") && request.method() === "PATCH"
  );
  await page.getByRole("button", { name: "Move 1" }).click();
  expect((await moveRequest).postDataJSON()).toEqual({
    paste_ids: ["sample-paste"],
    folder_id: null
  });

  await page.goto("/pastes?folder_id=5");
  await page.getByRole("link", { name: "New paste" }).click();
  await expect(page.getByLabel("Folder")).toHaveValue("5");
});

test("paste rows preserve content width and use selective metadata badges", async ({ page }) => {
  await mockApi(page, true, { items: [{ ...paste, folder_id: 5 }] });
  await page.goto("/pastes?folder_id=5");
  const layout = await page.locator(".paste-list .paste-row").evaluate(row => {
    const bounds = (selector: string) => {
      const rect = row.querySelector(selector)!.getBoundingClientRect();
      return { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width };
    };
    return {
      title: bounds(".paste-title"),
      preview: bounds(".paste-main-content>p"),
      footer: bounds(".paste-row-footer"),
      content: bounds(".paste-main-content"),
      actionsParent: row.querySelector(".row-actions")?.parentElement?.className,
      badges: row.querySelectorAll(".meta-badge").length,
      details: row.querySelectorAll(".meta-detail").length
    };
  });
  expect(layout.preview.top).toBeGreaterThanOrEqual(layout.title.bottom);
  expect(layout.footer.top).toBeGreaterThanOrEqual(layout.preview.bottom);
  expect(Math.abs(layout.title.width - layout.content.width)).toBeLessThan(1);
  expect(Math.abs(layout.preview.width - layout.content.width)).toBeLessThan(1);
  expect(layout.actionsParent).toContain("paste-row-footer");
  expect(layout.badges).toBe(2);
  expect(layout.details).toBeGreaterThanOrEqual(4);
  await expect(page.getByText("Folder: Scripts")).toBeVisible();
});

test("workspace boundaries align and the folder sidebar collapses persistently", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes");
  await expect(page.locator(".paste-list")).toBeVisible();
  await expect(page.getByRole("complementary", { name: "Paste folders" })).toBeVisible();
  const geometry = await page.evaluate(() => {
    const right = (selector: string) =>
      document.querySelector(selector)!.getBoundingClientRect().right;
    const counts = [...document.querySelectorAll(".folder-nav-row>a small")]
      .map(element => element.getBoundingClientRect().right);
    const longName = [...document.querySelectorAll<HTMLElement>(".folder-nav-row>a span")]
      .find(element => element.textContent === "sample-folder")!;
    return {
      rights: [
        right(".paste-workspace-main"),
        right(".page-heading"),
        right(".paste-filter-form"),
        right(".paste-filter-primary"),
        right(".paste-bulk-actions"),
        right(".paste-list")
      ],
      counts,
      longNameFits: longName.scrollWidth <= longName.clientWidth,
      contentWidth: document.querySelector(".paste-workspace-main")!.getBoundingClientRect().width
    };
  });
  expect(Math.max(...geometry.rights) - Math.min(...geometry.rights)).toBeLessThan(1);
  expect(Math.max(...geometry.counts) - Math.min(...geometry.counts)).toBeLessThan(1);
  expect(geometry.longNameFits).toBe(true);

  await page.getByRole("button", { name: "Collapse folders" }).click();
  await expect(page.locator(".paste-workspace")).toHaveClass(/folder-sidebar-collapsed/);
  await expect(page.getByRole("button", { name: "Expand folders" })).toBeVisible();
  const collapsedWidth = await page.locator(".paste-workspace-main").evaluate(
    element => element.getBoundingClientRect().width
  );
  expect(collapsedWidth).toBeGreaterThan(geometry.contentWidth);
  expect(await page.evaluate(() => localStorage.getItem("racebin.folderSidebarCollapsed"))).toBe("true");
  await page.addInitScript(() => {
    const observed: string[] = [];
    Object.assign(window, { __workspaceInitialClasses: observed });
    new MutationObserver(() => {
      if (observed.length) return;
      const workspace = document.querySelector(".paste-workspace");
      if (workspace) observed.push(workspace.className);
    }).observe(document, { childList: true, subtree: true });
  });
  await page.reload();
  await expect(page.getByRole("button", { name: "Expand folders" })).toBeVisible();
  expect(await page.evaluate(() =>
    (window as Window & { __workspaceInitialClasses: string[] }).__workspaceInitialClasses[0]
  )).toContain("folder-sidebar-collapsed");
});

test("folders can be created, renamed, and deleted from the workspace menu", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes?folder_id=5");

  page.once("dialog", dialog => dialog.accept("Notes"));
  const createRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/folders") && request.method() === "POST");
  await page.getByRole("button", { name: "New" }).click();
  expect((await createRequest).postDataJSON()).toEqual({ name: "Notes" });
  await expect(page).toHaveURL(/folder_id=6/);

  await page.goto("/pastes?folder_id=5");
  page.once("dialog", dialog => dialog.accept("Utilities"));
  const renameRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/folders/5") && request.method() === "PATCH");
  const manage = page.getByRole("button", { name: "Manage Scripts" });
  await manage.click();
  await expect(page.getByRole("menu")).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Rename" })).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("menuitem", { name: "Delete" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menu")).toHaveCount(0);
  await expect(manage).toBeFocused();
  await manage.click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  expect((await renameRequest).postDataJSON()).toEqual({ name: "Utilities" });
  await expect(page.getByRole("link", { name: /Utilities/ })).toBeVisible();

  page.once("dialog", dialog => dialog.accept());
  const deleteRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/folders/5") && request.method() === "DELETE");
  await page.getByRole("button", { name: "Manage Utilities" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await deleteRequest;
});

test("mobile folder and filter controls do not overflow", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await page.addInitScript(() =>
    localStorage.setItem("racebin.folderSidebarCollapsed", "true")
  );
  await mockApi(page, true);
  await page.goto("/pastes");
  await expect(page.locator(".folder-mobile-select select")).toBeVisible();
  await expect(page.getByRole("button", { name: "Expand folders" })).toBeHidden();
  const layout = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
    filterColumns: getComputedStyle(document.querySelector(".paste-filter-primary")!)
      .gridTemplateColumns.split(" ").length
  }));
  expect(layout.documentWidth).toBe(layout.viewportWidth);
  expect(layout.filterColumns).toBe(1);
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
