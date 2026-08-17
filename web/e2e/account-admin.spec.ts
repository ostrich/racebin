import { expect, test } from "@playwright/test";
import { mockApi } from "./support/mockApi";

test("account and admin ownership data render as structured controls", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: (value: string) => Object.assign(window, { __copiedText: value }) }
    });
  });
  await mockApi(page, true);
  await page.goto("/account");
  await expect(page.getByText("Automation")).toBeVisible();
  await expect(page.getByText(/paste:read, paste:write/)).toBeVisible();
  await expect(page.getByLabel("user:manage")).toBeVisible();
  await page.getByRole("link", { name: "Admin", exact: true }).click();
  await expect(page.getByRole("region", { name: "Site summary" })).toContainText("Users2");
  await expect(page.getByRole("region", { name: "Site summary" })).toContainText("Pastes4");
  await expect(page.getByRole("heading", { name: "Recent activity" })).toBeVisible();
  await expect(page.getByText("Everything looks normal.")).toBeVisible();
  await page.getByRole("link", { name: "Invitations", exact: true }).click();
  await expect(page.getByText("Redeemed by reader")).toBeVisible();
  const origin = new URL(page.url()).origin;
  await page.getByRole("button", { name: "Copy invitation" }).click();
  expect(await page.evaluate(() => (window as Window & { __copiedText: string }).__copiedText))
    .toBe(`${origin}/invitations/active-token`);
  await page.getByRole("button", { name: "Create invitation" }).click();
  await expect.poll(() => page.evaluate(
    () => (window as Window & { __copiedText: string }).__copiedText
  )).toBe(`${origin}/invitations/new-token`);
  await page.getByRole("link", { name: "API keys", exact: true }).click();
  await expect(page.getByText("test-admin · abcd", { exact: true })).toBeVisible();
  await expect(page.getByText("paste:write", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Site settings" })).toBeVisible();
  await expect(page.getByLabel("Site name")).toHaveValue("Racebin");
  await page.getByRole("link", { name: "Audit log", exact: true }).click();
  await expect(page.getByText("instance settings_changed")).toBeVisible();
  await page.goto("/admin/users/2");
  await page.getByLabel("Role").selectOption("admin");
  await page.getByRole("button", { name: "Save role" }).click();
  await page.getByLabel("Password").fill("correct password");
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByText("Role updated.")).toBeVisible();
});
