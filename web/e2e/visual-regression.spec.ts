import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

const screenshot = {
  animations: "disabled" as const,
  fullPage: true,
  maxDiffPixelRatio: 0.02,
};

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await mockApi(page, true, {
    items: Array.from({ length: 4 }, (_, index) => ({
      ...paste,
      id: `visual-paste-${index}`,
      title: `Example paste ${index + 1}`,
      folder_id: index < 2 ? 5 : null,
    })),
  });
});

for (const theme of ["auto", "light", "dark"] as const) {
  for (const viewport of [
    { name: "desktop", width: 1440, height: 900 },
    { name: "mobile", width: 390, height: 844 },
  ]) {
    test(`theme matrix: ${theme} ${viewport.name}`, { tag: "@visual" }, async ({ page }) => {
      await page.setViewportSize(viewport);
      await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
      await page.addInitScript(selectedTheme => {
        if (selectedTheme === "auto") localStorage.removeItem("racebin.colorTheme");
        else localStorage.setItem("racebin.colorTheme", selectedTheme);
      }, theme);
      await page.goto("/pastes");
      await expect(page.locator("html")).toHaveAttribute(
        "data-color-scheme",
        theme === "dark" ? "dark" : "light",
      );
      await expect(page).toHaveScreenshot(
        `theme-${theme}-${viewport.name}.png`,
        screenshot,
      );
    });
  }
}

test("desktop paste workspace", { tag: "@visual" }, async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/pastes");
  await expect(page).toHaveScreenshot("workspace-desktop.png", screenshot);
  await page.getByRole("button", { name: /^My pastes/ }).click();
  await expect(page).toHaveScreenshot("workspace-folders-desktop.png", screenshot);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Compact", exact: true }).click();
  await expect(page).toHaveScreenshot("workspace-compact-desktop.png", screenshot);
  await page.getByRole("button", { name: "Normal", exact: true }).click();
  await page.getByRole("button", { name: /^Filters/ }).click();
  await expect(page).toHaveScreenshot(
    "workspace-filters-desktop.png",
    screenshot,
  );
});

test("mobile paste workspace", { tag: "@visual" }, async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/pastes");
  await expect(page).toHaveScreenshot("workspace-mobile.png", screenshot);
});

test("public paste listing", { tag: "@visual" }, async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/explore");
  await expect(page).toHaveScreenshot("explore-desktop.png", screenshot);
});

test("paste editors", { tag: "@visual" }, async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/pastes/new");
  await expect(page).toHaveScreenshot("text-editor-desktop.png", screenshot);
  await page.locator(".form-grid select").first().selectOption("markdown");
  await expect(page.locator(".rich-text-editor")).toBeVisible();
  await expect(page).toHaveScreenshot(
    "rich-text-editor-desktop.png",
    screenshot,
  );
});

test("paste view and administration", { tag: "@visual" }, async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/pastes/sample-paste");
  await expect(page).toHaveScreenshot("paste-view-desktop.png", screenshot);
  await page.goto("/admin");
  await expect(page).toHaveScreenshot("admin-overview-desktop.png", screenshot);
  await page.goto("/admin/pastes");
  await expect(page).toHaveScreenshot("admin-pastes-desktop.png", screenshot);
  await page.goto("/admin/settings");
  await expect(page).toHaveScreenshot("admin-settings-desktop.png", screenshot);
  await page.goto("/admin/invitations");
  await expect(page).toHaveScreenshot("admin-invitations-desktop.png", screenshot);
  await page.getByRole("button", { name: "Create invitation" }).click();
  await expect(page).toHaveScreenshot("admin-invitation-dialog-desktop.png", screenshot);
  await page.keyboard.press("Escape");
  await page.getByRole("link", { name: "History" }).click();
  await expect(page).toHaveScreenshot("admin-invitation-history-desktop.png", screenshot);
});

test("dark account page", { tag: "@visual" }, async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  await page.goto("/account");
  await expect(page).toHaveScreenshot("account-dark-desktop.png", screenshot);
});

test("help page", { tag: "@visual" }, async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/help");
  await expect(page).toHaveScreenshot("help-desktop.png", screenshot);
});
