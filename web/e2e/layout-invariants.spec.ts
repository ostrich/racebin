import { expect, test } from "@playwright/test";
import { mockApi } from "./support/mockApi";

test.beforeEach(async ({ page }) => {
  await mockApi(page, true);
});

test("the bundled interface font is available", async ({ page }) => {
  await page.goto("/");
  const faces = await page.evaluate(async () =>
    (await document.fonts.load('16px "Racebin Inter"')).length
  );
  expect(faces).toBeGreaterThan(0);
});

test("primary pages do not overflow at supported widths", async ({ page }) => {
  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
    for (const path of [
      "/pastes",
      "/explore",
      "/pastes/new",
      "/account",
      "/admin/pastes",
      "/admin/users",
      "/admin/users/1",
      "/help",
    ]) {
      await page.goto(path);
      await expect
        .poll(() =>
          page.evaluate(
            () =>
              document.documentElement.scrollWidth <=
              document.documentElement.clientWidth,
          ), { message: `${path} should fit at ${viewport.width}px` }
        )
        .toBe(true);
    }
  }
});

test("workspace sections share a common content edge", async ({ page }) => {
  await page.goto("/pastes");
  const edges = await page.locator(".paste-workspace-main").evaluate((main) => {
    const bounds = (selector: string) => {
      const rect = main.querySelector(selector)!.getBoundingClientRect();
      return { left: rect.left, right: rect.right };
    };
    return [
      bounds(".page-heading"),
      bounds(".paste-filter-form"),
      bounds(".paste-selection-bar"),
      bounds(".paste-list"),
    ];
  });
  for (const edge of edges.slice(1)) {
    expect(Math.abs(edge.left - edges[0].left)).toBeLessThan(1);
    expect(Math.abs(edge.right - edges[0].right)).toBeLessThan(1);
  }
});

test("filter expansion preserves the search toolbar boundary", async ({
  page,
}) => {
  await page.goto("/pastes");
  const toolbar = page.locator(".paste-filter-toolbar");
  const before = await toolbar.evaluate(
    (element) => element.getBoundingClientRect().bottom,
  );
  await page.getByRole("button", { name: /^Filters/ }).click();
  const after = await toolbar.evaluate(
    (element) => element.getBoundingClientRect().bottom,
  );
  expect(after).toBe(before);
  await expect(toolbar).toHaveCSS("border-bottom-style", "solid");
});

test("standard form controls use the shared control height", async ({
  page,
}) => {
  await page.goto("/pastes/new");
  const heights = await page
    .locator(
      '.form-grid input:not([type="checkbox"]):not([type="radio"]):not([type="file"]), .form-grid select',
    )
    .evaluateAll((elements) =>
      elements.map((element) => element.getBoundingClientRect().height),
    );
  expect(new Set(heights)).toEqual(new Set([40]));
});
