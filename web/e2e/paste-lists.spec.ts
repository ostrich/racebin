import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

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

test("paste filters remain URL-addressable", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes");
  await page.getByLabel("Search").fill("javascript");
  await page.getByRole("button", { name: "Search" }).click();
  await expect(page).toHaveURL(/search=javascript/);
  await expect(page.getByLabel("Search")).toHaveValue("javascript");
});

test("paste list controls separate search, filters, and sorting", async ({ page }) => {
  await mockApi(page, true);

  for (const path of ["/pastes", "/explore", "/admin/pastes"]) {
    await page.goto(path);
    await expect(page.getByLabel("Search")).toBeVisible();
    await expect(page.getByRole("button", { name: /^Filters/ })).toBeVisible();
    await expect(page.getByRole("button", { name: "Sort: Newest" })).toBeVisible();
    await expect(page.getByLabel("Format")).toHaveCount(0);

    await page.getByRole("button", { name: /^Filters/ }).click();
    await expect(page.getByLabel("Format")).toBeVisible();
    if (path === "/explore") await expect(page.getByLabel("Visibility")).toHaveCount(0);
    else await expect(page.getByLabel("Visibility")).toBeVisible();
    if (path === "/admin/pastes") await expect(page.getByLabel("Owner ID")).toBeVisible();
    else await expect(page.getByLabel("Owner ID")).toHaveCount(0);
  }
});

test("search, filters, and sort preserve unrelated list state", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes?folder_id=5&sort=size&direction=desc&page=3");

  await page.getByLabel("Search").fill("example");
  await page.getByRole("button", { name: "Search" }).click();
  await expect(page).toHaveURL(/folder_id=5/);
  await expect(page).toHaveURL(/search=example/);
  await expect(page).toHaveURL(/sort=size/);
  await expect(page).not.toHaveURL(/page=3/);

  await page.getByRole("button", { name: /^Filters/ }).click();
  await page.getByLabel("Format").selectOption("text");
  await page.getByRole("button", { name: "Apply filters" }).click();
  await expect(page).toHaveURL(/content_kind=text/);
  await expect(page).toHaveURL(/search=example/);
  await expect(page).toHaveURL(/sort=size/);
  await expect(page.getByRole("button", { name: /Filters 1/ })).toBeVisible();
  await expect(page.getByRole("link", { name: /Format: Text/ })).toBeVisible();

  await page.getByRole("button", { name: "Sort: Largest" }).click();
  await page.getByRole("menuitemradio", { name: "Oldest" }).click();
  await expect(page).toHaveURL(/sort=created/);
  await expect(page).toHaveURL(/direction=asc/);
  await expect(page).toHaveURL(/content_kind=text/);
  await expect(page).toHaveURL(/folder_id=5/);

  await page.getByRole("link", { name: "Clear filters" }).click();
  await expect(page).not.toHaveURL(/content_kind/);
  await expect(page).toHaveURL(/search=example/);
  await expect(page).toHaveURL(/sort=created/);
  await expect(page).toHaveURL(/folder_id=5/);
});

test("sort menu supports keyboard selection and dismissal", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes");
  const sort = page.getByRole("button", { name: "Sort: Newest" });
  await sort.click();
  await expect(page.getByRole("menuitemradio", { name: "Newest" })).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("menuitemradio", { name: "Oldest" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(sort).toBeFocused();
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("paste checkboxes support range selection and indeterminate select-all", async ({ page }) => {
  const items = Array.from({ length: 6 }, (_, index) => ({
    ...paste,
    id: `range-paste-${index}`,
    title: `Range paste ${index + 1}`
  }));
  await mockApi(page, true, { items });
  await page.goto("/pastes");

  const first = page.getByRole("checkbox", { name: "Select Range paste 1" });
  const fourth = page.getByRole("checkbox", { name: "Select Range paste 4" });
  const selectAll = page.getByRole("checkbox", { name: "Select all on page" });
  await first.check();
  expect(await selectAll.evaluate(input => (input as HTMLInputElement).indeterminate)).toBe(true);
  await fourth.click({ modifiers: ["Shift"] });
  for (let index = 1; index <= 4; index += 1) {
    await expect(page.getByRole("checkbox", { name: `Select Range paste ${index}` })).toBeChecked();
  }
  await expect(page.getByRole("button", { name: "Move 4" })).toBeEnabled();

  await fourth.click({ modifiers: ["Shift"] });
  for (let index = 1; index <= 4; index += 1) {
    await expect(page.getByRole("checkbox", { name: `Select Range paste ${index}` })).not.toBeChecked();
  }
  expect(await selectAll.evaluate(input => (input as HTMLInputElement).indeterminate)).toBe(false);

  await selectAll.check();
  await expect(selectAll).toBeChecked();
  await expect(page.getByRole("button", { name: "Move 6" })).toBeEnabled();
  await selectAll.uncheck();
  await expect(page.locator(".move-selected-button")).toBeDisabled();

  await first.check();
  await fourth.click({ modifiers: ["Shift"] });
  const moveRequest = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/pastes") && request.method() === "PATCH");
  await page.getByRole("button", { name: "Move 4" }).click();
  expect((await moveRequest).postDataJSON()).toEqual({
    ids: ["range-paste-0", "range-paste-1", "range-paste-2", "range-paste-3"],
    folder_id: null
  });
});

test("bulk controls retain their geometry as selection changes", async ({ page }) => {
  const items = Array.from({ length: 12 }, (_, index) => ({
    ...paste,
    id: `geometry-paste-${index}`,
    title: `Geometry paste ${index + 1}`
  }));
  await mockApi(page, true, { items });
  await page.goto("/pastes");

  const geometry = () => page.locator(".paste-selection-bar").evaluate(element => {
    const bounds = (selector: string) => {
      const rect = element.querySelector(selector)!.getBoundingClientRect();
      return { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom, width: rect.width };
    };
    return {
      view: bounds(".paste-view-switch"),
      count: bounds(".result-count"),
      destination: bounds("select"),
      move: bounds(".move-selected-button"),
      selectAll: bounds(".select-all-pastes")
    };
  });
  const empty = await geometry();
  await page.getByRole("checkbox", { name: "Select Geometry paste 1", exact: true }).check();
  const one = await geometry();
  await page.getByRole("checkbox", { name: "Select all on page" }).check();
  const all = await geometry();
  expect(one).toEqual(empty);
  expect(all).toEqual(empty);
  expect(empty.view.top).toBe(empty.destination.top);
  expect(empty.view.bottom).toBe(empty.destination.bottom);
  expect(empty.count.top).toBe(empty.selectAll.top);
  expect(empty.selectAll.top).toBeGreaterThanOrEqual(empty.move.bottom);
});

test("compact view is persistent and preserves paste selection", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes");

  const normal = page.getByRole("button", { name: "Normal", exact: true });
  const compact = page.getByRole("button", { name: "Compact", exact: true });
  const pasteCheckbox = page.getByRole("checkbox", {
    name: "Select JavaScript example"
  });
  await expect(normal).toHaveAttribute("aria-pressed", "true");
  await pasteCheckbox.check();
  await compact.click();

  await expect(compact).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".paste-list")).toHaveClass(/compact/);
  await expect(pasteCheckbox).toBeChecked();
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Copy link" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Edit" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete" })).toBeVisible();
  await expect(page.getByText("const answer = 42; console.log(answer);")).toBeHidden();
  await expect(page.getByText("1 attachment")).toBeHidden();
  expect(await page.evaluate(() => localStorage.getItem("racebin.pasteListView"))).toBe("compact");

  await page.reload();
  await expect(compact).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".paste-list")).toHaveClass(/compact/);
});

test("compact view remains usable without horizontal overflow on mobile", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mockApi(page, true);
  await page.goto("/pastes");
  await page.getByRole("button", { name: "Compact", exact: true }).click();

  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth))
    .toBe(true);
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  await expect(page.getByLabel("Paste view")).toBeVisible();
});

test("selection and its range anchor reset with list navigation", async ({ page }) => {
  const items = Array.from({ length: 4 }, (_, index) => ({
    ...paste,
    id: `reset-paste-${index}`,
    title: `Reset paste ${index + 1}`
  }));
  await mockApi(page, true, { items });
  await page.goto("/pastes");
  await page.getByRole("checkbox", { name: "Select Reset paste 1" }).check();
  await page.getByRole("button", { name: "Sort: Newest" }).click();
  await page.getByRole("menuitemradio", { name: "Oldest" }).click();
  await expect(page.locator(".move-selected-button")).toBeDisabled();
  await page.getByRole("checkbox", { name: "Select Reset paste 3" }).click({ modifiers: ["Shift"] });
  await expect(page.getByRole("button", { name: "Move 1" })).toBeEnabled();
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
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  await page.evaluate(() => {
    Object.assign(window, { __retainedList: document.querySelector(".paste-workspace") });
  });
  await page.getByLabel("Search").fill("filtered");
  await page.getByRole("button", { name: "Search" }).click();
  await page.waitForTimeout(50);
  await expect(page.getByRole("complementary", { name: "Paste folders" })).toBeVisible();
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  expect(await page.evaluate(() =>
    (window as Window & { __retainedList: Element }).__retainedList
      === document.querySelector(".paste-workspace")
  )).toBe(true);
  await expect(page.getByRole("link", { name: "Filtered result" })).toBeVisible();

  await page.goto("/admin/pastes");
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  await page.evaluate(() => {
    Object.assign(window, { __retainedAdmin: document.querySelector("main > section") });
  });
  await page.getByLabel("Search").fill("filtered");
  await page.getByRole("button", { name: "Search" }).click();
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
