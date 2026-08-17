import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

test("folders filter the workspace and carry into new pastes", async ({ page }) => {
  await mockApi(page, true, { items: [{ ...paste, folder_id: 5 }] });
  await page.goto("/pastes?folder_id=5");
  await expect(page.getByRole("heading", { name: "Scripts" })).toBeVisible();
  await page.getByRole("button", { name: /^Scripts/ }).click();
  await expect(page.getByRole("dialog", { name: "Browse folders" })
    .getByRole("button", { name: /^Scripts 1$/ })).toHaveClass(/current/);
  await page.keyboard.press("Escape");
  const selectedPaste = page.getByRole("checkbox", { name: /Select JavaScript example/ });
  await selectedPaste.check();
  const moveRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/pastes") && request.method() === "PATCH"
  );
  await page.getByRole("button", { name: "Move 1" }).click();
  await page.getByRole("dialog", { name: "Move selected pastes" })
    .getByRole("button", { name: /Uncategorized/ }).click();
  expect((await moveRequest).postDataJSON()).toEqual({
    ids: ["sample-paste"],
    folder_id: null
  });

  await page.goto("/pastes?folder_id=5");
  await page.getByRole("link", { name: "New paste" }).click();
  await expect(page.getByLabel("Folder")).toHaveValue("5");
});

test("move destinations align folder counts without browse-action space", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes");
  await page.getByRole("checkbox", { name: /Select JavaScript example/ }).check();
  await page.getByRole("button", { name: "Move 1" }).click();

  const countEdges = await page.getByRole("dialog", { name: "Move selected pastes" })
    .locator(".folder-picker-choice small")
    .evaluateAll(counts => counts.map(count => count.getBoundingClientRect().right));
  expect(countEdges.length).toBeGreaterThan(1);
  expect(Math.max(...countEdges) - Math.min(...countEdges)).toBeLessThan(1);
});

test("workspace boundaries align without reserving a folder sidebar", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes");
  await expect(page.locator(".paste-list")).toBeVisible();
  const geometry = await page.evaluate(() => {
    const right = (selector: string) =>
      document.querySelector(selector)!.getBoundingClientRect().right;
    return {
      rights: [
        right(".paste-workspace-main"),
        right(".page-heading"),
        right(".paste-filter-form"),
        right(".paste-filter-toolbar"),
        right(".paste-selection-bar"),
        right(".paste-list")
      ],
      workspaceLeft: document.querySelector(".paste-workspace")!.getBoundingClientRect().left,
      mainLeft: document.querySelector(".paste-workspace-main")!.getBoundingClientRect().left
    };
  });
  expect(Math.max(...geometry.rights) - Math.min(...geometry.rights)).toBeLessThan(1);
  expect(geometry.mainLeft).toBe(geometry.workspaceLeft);
});

test("cold workspace navigation keeps main content in its final position", async ({ page }) => {
  await mockApi(page, true, { delay: 200 });
  await page.goto("/pastes/new");
  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page.getByText("Loading pastes…")).toBeVisible();

  const loadingGeometry = await page.locator(".paste-workspace").evaluate(workspace => {
    const main = workspace.querySelector<HTMLElement>(".paste-workspace-main")!;
    const bounds = workspace.getBoundingClientRect();
    const mainBounds = main.getBoundingClientRect();
    return {
      left: mainBounds.left,
      width: mainBounds.width,
      sharesWorkspaceEdge: mainBounds.left === bounds.left,
      occupiesMainColumn: mainBounds.width > bounds.width / 2
    };
  });
  expect(loadingGeometry.sharesWorkspaceEdge).toBe(true);
  expect(loadingGeometry.occupiesMainColumn).toBe(true);

  await expect(page.locator(".paste-list")).toBeVisible();
  const loadedGeometry = await page.locator(".paste-workspace-main").evaluate(main => ({
    left: main.getBoundingClientRect().left,
    width: main.getBoundingClientRect().width
  }));
  expect(loadedGeometry.left).toBeCloseTo(loadingGeometry.left, 5);
  expect(loadedGeometry.width).toBeCloseTo(loadingGeometry.width, 5);
});

test("folders can be searched, created, renamed, and deleted from the picker", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes?folder_id=5");

  const createRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/folders") && request.method() === "POST");
  await page.getByRole("button", { name: /^Scripts/ }).click();
  const picker = page.getByRole("dialog", { name: "Browse folders" });
  await picker.getByPlaceholder("Filter folders").fill("sample");
  await expect(picker.getByRole("button", { name: /^sample-folder 18$/ })).toBeVisible();
  await expect(picker.getByRole("button", { name: /^Scripts 1$/ })).toBeHidden();
  await picker.getByRole("button", { name: "New folder" }).click();
  const dialogGeometry = await page.getByRole("dialog").evaluate(dialog => {
    const input = dialog.querySelector("input")!.getBoundingClientRect();
    const actions = dialog.querySelector(".actions")!.getBoundingClientRect();
    return { inputBottom: input.bottom, actionsTop: actions.top };
  });
  expect(dialogGeometry.actionsTop - dialogGeometry.inputBottom).toBeGreaterThanOrEqual(8);
  await page.getByRole("dialog").getByLabel("Folder name").fill("Notes");
  await page.getByRole("button", { name: "Create folder" }).click();
  expect((await createRequest).postDataJSON()).toEqual({ name: "Notes" });
  await expect(page).toHaveURL(/folder_id=6/);

  await page.goto("/pastes?folder_id=5");
  const renameRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/folders/5") && request.method() === "PATCH");
  await page.getByRole("button", { name: /^Scripts/ }).click();
  const manage = page.getByRole("button", { name: "Manage Scripts" });
  await manage.click();
  await page.getByRole("button", { name: "Rename" }).click();
  await page.getByRole("dialog").getByLabel("Folder name").fill("Utilities");
  await page.getByRole("button", { name: "Rename", exact: true }).click();
  expect((await renameRequest).postDataJSON()).toEqual({ name: "Utilities" });
  await page.getByRole("button", { name: /^Utilities/ }).click();
  await expect(page.getByRole("dialog", { name: "Browse folders" })
    .getByRole("button", { name: /^Utilities 1$/ })).toBeVisible();

  const deleteRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/folders/5") && request.method() === "DELETE");
  await page.getByRole("button", { name: "Manage Utilities" }).click();
  await page.getByRole("dialog", { name: "Browse folders" })
    .getByRole("button", { name: "Delete", exact: true }).click();
  await page.getByRole("button", { name: "Delete folder" }).click();
  await deleteRequest;
});

test("mobile folder and filter controls do not overflow", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await mockApi(page, true);
  await page.goto("/pastes");
  await expect(page.getByRole("button", { name: /^My pastes/ })).toBeVisible();
  const layout = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
    searchColumns: getComputedStyle(document.querySelector(".paste-search")!)
      .gridTemplateColumns.split(" ").length
  }));
  expect(layout.documentWidth).toBe(layout.viewportWidth);
  expect(layout.searchColumns).toBe(1);
});
