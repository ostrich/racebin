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
