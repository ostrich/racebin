import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

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
        right(".paste-filter-toolbar"),
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

test("folder sidebar remains fixed at its initial position while scrolling", async ({ page }) => {
  const items = Array.from({ length: 40 }, (_, index) => ({
    ...paste,
    id: `scroll-paste-${index}`,
    title: `Scroll paste ${index + 1}`
  }));
  await mockApi(page, true, { items });
  await page.goto("/pastes");
  const sidebar = page.getByRole("complementary", { name: "Paste folders" });
  const initialTop = await sidebar.evaluate(element => element.getBoundingClientRect().top);
  await page.evaluate(() => window.scrollTo(0, 300));
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(300);
  const scrolledTop = await sidebar.evaluate(element => element.getBoundingClientRect().top);
  expect(scrolledTop).toBe(initialTop);
});

test("folders can be created, renamed, and deleted from the workspace menu", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes?folder_id=5");

  page.once("dialog", dialog => dialog.accept("Notes"));
  const createRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/folders") && request.method() === "POST");
  await page.getByRole("button", { name: "New", exact: true }).click();
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
    searchColumns: getComputedStyle(document.querySelector(".paste-search")!)
      .gridTemplateColumns.split(" ").length
  }));
  expect(layout.documentWidth).toBe(layout.viewportWidth);
  expect(layout.searchColumns).toBe(1);
});
