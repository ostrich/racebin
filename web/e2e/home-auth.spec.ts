import { expect, test } from "@playwright/test";
import { mockApi } from "./support/mockApi";

test("renders the public homepage and paste viewer", async ({ page }) => {
  await mockApi(page);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Racebin" })).toBeVisible();
  await page.getByRole("link", { name: "JavaScript example" }).click();
  await expect(page).toHaveURL(/\/pastes\/sample-paste$/);
  await expect(page.locator(".line-numbers")).toHaveText("1\n2");
  await expect(page.locator("code.hljs")).toContainText("const answer");
  await expect(page.getByRole("link", { name: /example.txt/ })).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "Wrap" })).toHaveCount(0);
});

test("plain home presents only login while public routes remain available", async ({ page }) => {
  let homepagePasteRequests = 0;
  page.on("request", request => {
    const url = new URL(request.url());
    if (url.pathname === "/api/v1/pastes" && page.url().endsWith("/")) homepagePasteRequests += 1;
  });
  await mockApi(page, false, { plainHome: true });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Log in" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Racebin" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Explore" })).toHaveCount(0);
  await expect(page.getByRole("link", { name: "Log in" })).toHaveCount(0);
  await expect(page.getByText("Recently shared")).toHaveCount(0);
  expect(homepagePasteRequests).toBe(0);

  await page.goto("/explore");
  await expect(page.getByRole("heading", { name: "Explore" })).toBeVisible();
  await expect(page.getByRole("link", { name: "JavaScript example" })).toBeVisible();
  await page.getByRole("link", { name: "JavaScript example" }).click();
  await expect(page).toHaveURL(/\/pastes\/sample-paste$/);
});

test("plain-home login and authenticated homepage retain normal behavior", async ({ page }) => {
  await mockApi(page, false, { plainHome: true });
  await page.goto("/");
  await page.getByLabel("Username").fill("test-admin");
  await page.getByLabel("Password").fill("password");
  await page.getByRole("button", { name: "Log in" }).click();
  await expect(page).toHaveURL(/\/pastes$/);
  await expect(page.getByRole("heading", { name: "My pastes" })).toBeVisible();
  await page.getByRole("link", { name: "Racebin" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("heading", { name: "New paste" })).toBeVisible();
});
