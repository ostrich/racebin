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
  await page.getByRole("link", { name: "Admin", exact: true }).click();
  await page.getByRole("button", { name: /Invitations/ }).click();
  await expect(page.getByText("Redeemed by reader")).toBeVisible();
  const origin = new URL(page.url()).origin;
  await page.getByRole("button", { name: "Copy invitation active" }).click();
  expect(await page.evaluate(() => (window as Window & { __copiedText: string }).__copiedText))
    .toBe(`${origin}/invitations/active-token`);
  await page.getByRole("button", { name: "Create invitation" }).click();
  await expect.poll(() => page.evaluate(
    () => (window as Window & { __copiedText: string }).__copiedText
  )).toBe(`${origin}/invitations/new-token`);
  await page.getByRole("button", { name: /API keys/ }).click();
  await expect(page.getByText("Owner: test-admin")).toBeVisible();
  await expect(page.getByLabel("Privileges").getByText("paste:write")).toBeVisible();
});
