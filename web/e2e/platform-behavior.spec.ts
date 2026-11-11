import { expect, test } from "@playwright/test";
import { mockApi } from "./support/mockApi";

test.beforeEach(async ({ page }) => {
  await mockApi(page, true);
});

test("folder popover dismisses natively and restores keyboard focus", async ({ page }) => {
  await page.goto("/pastes");
  const trigger = page.locator(".folder-picker-trigger").first();
  await trigger.click();
  const popup = page.getByRole("dialog", { name: "Browse folders" });
  await expect(popup).toBeVisible();
  await expect(page.getByRole("searchbox", { name: "Filter folders" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(popup).not.toBeVisible();
  await expect(trigger).toBeFocused();
  await trigger.click();
  await page.getByRole("heading", { name: "My pastes", exact: true }).click();
  await expect(popup).not.toBeVisible();
});

test("primary pages reflow with enlarged default text", async ({ page }) => {
  await page.addInitScript(() => {
    document.addEventListener("DOMContentLoaded", () => {
      document.documentElement.style.fontSize = "32px";
    });
  });
  await page.setViewportSize({ width: 780, height: 1000 });
  for (const route of ["/pastes", "/explore", "/pastes/new", "/account", "/help"]) {
    await page.goto(route);
    await expect(page.locator("h1")).toBeVisible();
    const overflow = await page.evaluate(() =>
      [...document.querySelectorAll<HTMLElement>("body *")]
        .filter((element) => element.getBoundingClientRect().right > innerWidth + 1)
        .map((element) => ({
          tag: element.tagName,
          class: element.className,
          width: element.getBoundingClientRect().width
        }))
        .slice(0, 12)
    );
    expect(
      await page.locator("html").evaluate((root) => root.scrollWidth <= root.clientWidth),
      `${route}: ${JSON.stringify(overflow)}`
    ).toBe(true);
    expect(
      await page.locator("h1").evaluate((heading) => parseFloat(getComputedStyle(heading).fontSize))
    ).toBe(68);
  }
});
