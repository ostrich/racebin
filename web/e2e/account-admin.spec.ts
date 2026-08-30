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
  await expect(page.getByText("No administrative follow-up is needed.")).toBeVisible();
  await page.getByRole("link", { name: "Invitations", exact: true }).click();
  await expect(page.getByText("For a new teammate")).toBeVisible();
  const origin = new URL(page.url()).origin;
  await page.getByRole("button", { name: "Copy invitation link" }).click();
  expect(
    await page.evaluate(() => (window as Window & { __copiedText: string }).__copiedText)
  ).toBe(`${origin}/invitations/active-token`);
  await page.getByRole("button", { name: "Create invitation" }).click();
  const invitationDialog = page.getByRole("dialog");
  await invitationDialog.getByLabel("Private note Optional").fill("For an invited user");
  await invitationDialog.getByRole("button", { name: "Create invitation" }).click();
  await expect(page.getByRole("heading", { name: "Invitation created" })).toBeVisible();
  await page.getByRole("button", { name: "Copy link" }).click();
  await expect
    .poll(() => page.evaluate(() => (window as Window & { __copiedText: string }).__copiedText))
    .toBe(`${origin}/invitations/new-token`);
  await page.getByRole("button", { name: "Done" }).click();
  await page.getByRole("button", { name: "Edit private note" }).click();
  await page.getByLabel("Private note Optional").fill("Updated invitation note");
  const noteUpdate = page.waitForRequest(
    (request) =>
      request.method() === "PATCH" && request.url().endsWith("/api/v1/admin/invitations/4")
  );
  await page.getByRole("button", { name: "Save note" }).click();
  expect((await noteUpdate).postDataJSON()).toEqual({ comment: "Updated invitation note" });
  await page.getByRole("link", { name: "History" }).click();
  await expect(page.getByText(/Redeemed by reader/)).toBeVisible();
  await page.getByLabel("Status").selectOption("redeemed");
  const invitationRows = page.locator(".invitation-row");
  await expect(invitationRows).toHaveCount(2);
  const rowGeometry = await invitationRows.evaluateAll((rows) =>
    rows.map((row) => {
      const lifecycle = row.querySelector(".invitation-lifecycle")!.getBoundingClientRect();
      const actions = row.querySelector(".row-actions")!.getBoundingClientRect();
      return { lifecycleLeft: lifecycle.left, actionsRight: actions.right };
    })
  );
  expect(rowGeometry[0].lifecycleLeft).toBe(rowGeometry[1].lifecycleLeft);
  expect(rowGeometry[0].actionsRight).toBe(rowGeometry[1].actionsRight);
  await page.getByRole("link", { name: "API keys", exact: true }).click();
  await expect(page.getByText("test-admin · abcd", { exact: true })).toBeVisible();
  await expect(page.getByText("paste:write", { exact: true })).toBeVisible();
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Site settings" })).toBeVisible();
  await expect(page.getByLabel("Site name")).toHaveValue("Racebin");
  await expect(page.getByLabel("Language")).toHaveText(/Auto detect/);
  await page.getByRole("link", { name: "Audit log", exact: true }).click();
  await expect(page.getByText("instance settings_changed")).toBeVisible();
  await page.goto("/admin/users/2");
  await page.getByLabel("Role").selectOption("admin");
  await page.getByRole("button", { name: "Save role" }).click();
  const passwordDialog = page.getByRole("dialog", { name: "Confirm your password" });
  await passwordDialog.getByLabel("Password", { exact: true }).fill("correct password");
  await passwordDialog.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByText("Role updated.")).toBeVisible();
});

test("administrative lists keep server-side filters and pagination in the URL", async ({
  page
}) => {
  await mockApi(page, true);
  const requests: URL[] = [];
  await page.route("**/api/v1/admin/users?*", async (route) => {
    const url = new URL(route.request().url());
    requests.push(url);
    const pageNumber = Number(url.searchParams.get("page") ?? 1);
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        items: [
          {
            id: pageNumber,
            username: `user-${pageNumber}`,
            role: "user",
            enabled: true,
            password_change_required: false,
            created_at: "2023-11-14T22:13:20Z",
            last_login_at: null,
            paste_count: 0,
            storage_bytes: 0,
            active_session_count: 0,
            api_key_count: 0,
            active_api_key_count: 0
          }
        ],
        pagination: { page: pageNumber, page_size: 25, total_items: 26, total_pages: 2 }
      })
    });
  });

  await page.goto("/admin/users?search=user");
  await expect(page.getByText("Page 1 of 2")).toBeVisible();
  expect(requests.at(-1)?.searchParams.get("search")).toBe("user");
  expect(requests.at(-1)?.searchParams.get("page_size")).toBe("25");
  await page.getByRole("link", { name: "Next" }).click();
  await expect(page).toHaveURL(/search=user.*page=2|page=2.*search=user/);
  await expect(page.getByText("user-2")).toBeVisible();
  await page.getByLabel("Status").selectOption("disabled");
  await expect(page).toHaveURL(/status=disabled/);
  expect(new URL(page.url()).searchParams.has("page")).toBe(false);
});
