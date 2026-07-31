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
  await page.getByRole("button", { name: "Copy" }).first().click();
  expect(await page.evaluate(() => (window as Window & { __copiedText: string }).__copiedText))
    .toContain(`${new URL(page.url()).origin}/api/v1/pastes`);
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
  await expect(page.getByText("4.0 KiB")).toBeVisible();
  await page.getByRole("link", { name: "Manage" }).click();
  await expect(page.getByRole("heading", { name: "test-admin" })).toBeVisible();
  await page.getByRole("button", { name: /Create and copy reset link/ }).click();
  expect(await page.evaluate(() => (window as Window & { __copiedText: string }).__copiedText))
    .toBe(`${new URL(page.url()).origin}/password-reset/sample-reset-token`);
});

test("password recovery validates and submits a new password", async ({ page }) => {
  await mockApi(page, false);
  await page.goto("/password-reset/sample-reset-token");
  await page.getByLabel("New password").fill("a replacement password");
  await page.getByLabel("Confirm password").fill("a replacement password");
  await page.getByRole("button", { name: "Reset password" }).click();
  await expect(page.getByRole("heading", { name: "Password reset" })).toBeVisible();
});
