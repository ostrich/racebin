import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

test("created paste is handed directly to the viewer", async ({ page }) => {
  await mockApi(page, true);
  const reads: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.endsWith("/reads")) reads.push(request.url());
  });
  await page.goto("/pastes/new");
  await page.getByRole("textbox", { name: "Paste content" }).fill("new content");
  await page.getByRole("button", { name: "Create paste" }).click();

  await expect(page).toHaveURL(/\/pastes\/sample-paste$/);
  await expect(page.getByRole("heading", { name: "JavaScript example" })).toBeVisible();
  expect(reads).toEqual([]);
});

test("folders filter the workspace and carry into new pastes", async ({ page }) => {
  await mockApi(page, true, { items: [{ ...paste, folder_id: 5 }] });
  await page.goto("/pastes?folder_id=5");
  await expect(page.getByRole("heading", { name: "Scripts" })).toBeVisible();
  await page.getByRole("button", { name: /^Scripts/ }).click();
  await expect(
    page
      .getByRole("dialog", { name: "Browse folders" })
      .getByRole("button", { name: /^Scripts 1$/ })
  ).toHaveClass(/current/);
  await page.keyboard.press("Escape");
  const selectedPaste = page.getByRole("checkbox", { name: /Select JavaScript example/ });
  await selectedPaste.check();
  const moveRequest = page.waitForRequest(
    (request) => request.url().endsWith("/api/v1/pastes") && request.method() === "PATCH"
  );
  await page.getByRole("button", { name: "Move 1" }).click();
  await page
    .getByRole("dialog", { name: "Move selected pastes" })
    .getByRole("button", { name: /Uncategorized/ })
    .click();
  expect((await moveRequest).postDataJSON()).toEqual({
    ids: ["sample-paste"],
    folder_id: null
  });

  await page.goto("/pastes?folder_id=5");
  await page.getByRole("link", { name: "New paste" }).click();
  await expect(page.getByLabel("Folder")).toHaveValue("5");
});
