import { expect, test } from "@playwright/test";

test("compiled frontend creates and reads a paste through a disposable backend", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Username").fill("test-admin");
  await page.getByLabel("Password").fill("correct horse battery staple");
  await page.getByRole("button", { name: "Log in" }).click();
  await expect(page).toHaveURL(/\/pastes$/);

  await page.getByRole("link", { name: "New paste", exact: true }).click();
  await page.getByLabel("Title").fill("Disposable stack smoke test");
  await page.getByLabel("Paste content").fill("const verified = true;");
  await page.getByRole("button", { name: "Create paste" }).click();

  await expect(page).toHaveURL(/\/pastes\/[^/]+$/);
  await expect(page.getByRole("heading", { name: "Disposable stack smoke test" })).toBeVisible();
  await expect(page.locator("code.hljs")).toContainText("const verified = true;");
});
