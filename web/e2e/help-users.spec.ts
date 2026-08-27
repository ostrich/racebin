import { expect, test } from "@playwright/test";
import { mockApi } from "./support/mockApi";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: (value: string) => Object.assign(window, { __copiedText: value }) }
    });
  });
});

test("signed-in help explains API keys using the current installation", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/help");
  await expect(page.getByRole("heading", { name: "Using Racebin" })).toBeVisible();
  await expect(page.getByText("Authorization: Bearer $RACEBIN_API_KEY", { exact: false }).first()).toBeVisible();
  await expect(page.getByRole("heading", { name: "Update a paste" })).toBeVisible();
  await expect(page.getByText("curl -X PATCH", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "Copy" }).first().click();
  expect(await page.evaluate(() => (window as Window & { __copiedText: string }).__copiedText))
    .toContain(`${new URL(page.url()).origin}/api/v1/pastes`);
});

test("site basics explains creation, visibility, limits, and management", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/help#basics");
  const basics = page.locator("#basics");
  await expect(basics.getByRole("heading", { name: "Create a paste" })).toBeVisible();
  await expect(basics.locator("dt", { hasText: "Public" })).toBeVisible();
  await expect(basics.getByText("The URL is access, not a password.")).toBeVisible();
  await expect(basics.getByRole("heading", { name: "Control how long it remains available" })).toBeVisible();
  await expect(basics.getByRole("heading", { name: "Organize and manage" })).toBeVisible();
  await expect(basics.getByRole("heading", { name: "Before sharing sensitive information" })).toBeVisible();
});

test("help navigation aligns with its content and clears the sticky header", async ({ page }) => {
  await mockApi(page, true);
  await page.setViewportSize({ width: 1440, height: 700 });
  await page.goto("/help");
  const navigation = page.getByRole("complementary", { name: "Help topics" });
  const content = page.locator(".help-content > .panel").first();
  await expect(navigation.getByRole("link")).toHaveText([
    "Site basics",
    "API keys",
    "Command examples",
    "Key privileges"
  ]);
  await expect(page.locator(".help-content > .panel > h2")).toHaveText([
    "Site basics",
    "API keys",
    "Command examples",
    "Key privileges"
  ]);
  const initial = await Promise.all([
    navigation.evaluate(element => element.getBoundingClientRect().top),
    content.evaluate(element => element.getBoundingClientRect().top)
  ]);
  expect(Math.abs(initial[0] - initial[1])).toBeLessThan(1);

  await page.evaluate(() => window.scrollTo(0, 400));
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(400);
  const stickyPosition = await navigation.evaluate(element => ({
    actual: element.getBoundingClientRect().top,
    expected: Number.parseFloat(getComputedStyle(element).top),
    headerBottom: document.querySelector("header")!.getBoundingClientRect().bottom,
    standardGap: Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--space-4"))
  }));
  expect(stickyPosition.actual).toBe(stickyPosition.expected);
  expect(stickyPosition.actual - stickyPosition.headerBottom).toBe(stickyPosition.standardGap);

  await page.getByRole("link", { name: "Key privileges" }).click();
  await expect(page).toHaveURL(/\/help#scopes$/);
  await expect(page.locator("#scopes")).toBeInViewport();
  await expect.poll(() => page.evaluate(() => window.scrollY)).toBeGreaterThan(1_000);
});

test("anonymous visitors are sent from help to login", async ({ page }) => {
  await mockApi(page, false);
  await page.goto("/help");
  await expect(page).toHaveURL(/\/login$/);
  await expect(page.getByRole("heading", { name: "Log in" })).toBeVisible();
});

test("administrator can inspect a user and copy a recovery link", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/admin/users");
  await expect(page.getByRole("heading", { name: "Users" })).toBeVisible();
  const administrator = page.getByRole("row").filter({ hasText: "test-admin" });
  await expect(administrator.getByText("4.0 KiB")).toBeVisible();
  await administrator.getByRole("link", { name: "Manage" }).click();
  await expect(page.getByRole("heading", { name: "test-admin" })).toBeVisible();
  await page.getByRole("button", { name: /Create and copy reset link/ }).click();
  await expect(page.getByText("Password reset link copied.")).toBeVisible();
  await expect.poll(() => page.evaluate(() =>
    (window as Window & { __copiedText?: string }).__copiedText
  )).toBe(`${new URL(page.url()).origin}/password-reset/sample-reset-token`);
  await page.getByRole("link", { name: "View pastes" }).click();
  await expect(page).toHaveURL(/\/admin\/pastes\?owner_id=1$/);
  await expect(page.getByRole("heading", { name: "All pastes" })).toBeVisible();
  await expect(page.locator(".admin-paste-head > span")).toHaveText([/Paste\s*1 total/, "Owner", "Details", "Actions"]);
  await expect(page.locator(".admin-paste-row .paste-meta time").first()).toBeVisible();
  await page.getByRole("button", { name: /^Filters/ }).click();
  await expect(page.getByLabel("Owner ID")).toHaveValue("1");
});

test("user administration follows shared spacing and field primitives", async ({ page }) => {
  await mockApi(page, true);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/admin/users/1");
  const gaps = await page.evaluate(() => {
    const box = (selector: string) => document.querySelector(selector)!.getBoundingClientRect();
    const metrics = box(".admin-user-metrics");
    const panels = box(".admin-user-panels");
    const left = box(".admin-user-panels > .panel:first-child");
    const right = box(".admin-user-panels > .panel:last-child");
    const heading = box(".admin-user-panels > .panel:first-child h2");
    const description = box(".admin-user-panels > .panel:first-child p");
    return {
      section: panels.top - metrics.bottom,
      columns: right.left - left.right,
      panelContent: description.top - heading.bottom
    };
  });
  expect(gaps).toEqual({ section: 16, columns: 16, panelContent: 16 });
});

test("password recovery validates and submits a new password", async ({ page }) => {
  await mockApi(page, false);
  await page.goto("/password-reset/sample-reset-token");
  await page.getByLabel("New password").fill("a replacement password");
  await page.getByLabel("Confirm password").fill("a replacement password");
  await page.getByRole("button", { name: "Reset password" }).click();
  await expect(page.getByRole("heading", { name: "Password reset" })).toBeVisible();
});
