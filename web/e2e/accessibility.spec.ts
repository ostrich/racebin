import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import { mockApi } from "./support/mockApi";

async function expectNoAccessibilityViolations(page: Page): Promise<void> {
  const { violations } = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
    .analyze();

  expect(
    violations.map(({ id, impact, nodes }) => ({
      id,
      impact,
      targets: nodes.map((node) => node.target.join(" "))
    }))
  ).toEqual([]);
}

test("anonymous application shell and login are accessible", async ({ page }) => {
  await mockApi(page, false);
  for (const path of ["/", "/login"]) {
    await page.goto(path);
    await expect(page.locator("main")).not.toBeEmpty();
    await expectNoAccessibilityViolations(page);
  }
});

test("primary authenticated workflows are accessible", async ({ page }) => {
  await mockApi(page, true);
  for (const path of [
    "/pastes",
    "/pastes/new",
    "/pastes/sample-paste",
    "/admin",
    "/admin/invitations"
  ]) {
    await page.goto(path);
    await expect(page.locator("main")).not.toBeEmpty();
    await expectNoAccessibilityViolations(page);
  }
  await page.getByRole("button", { name: "Create invitation" }).click();
  await expectNoAccessibilityViolations(page);
});

test("opened custom controls expose accessible interaction state", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const language = page.getByRole("combobox", { name: /Language/ });
  await language.click();
  await page.keyboard.press("ArrowDown");
  await expect(language).toHaveAttribute("aria-activedescendant", /language-option-/);
  await expectNoAccessibilityViolations(page);

  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const visual = page.getByRole("button", { name: "Visual", exact: true });
  const markdown = page.getByRole("button", { name: "Markdown", exact: true });
  await expect(visual).toHaveAttribute("aria-pressed", "true");
  await expect(markdown).toHaveAttribute("aria-pressed", "false");
  await page.getByRole("button", { name: "Insert table" }).click();
  const grid = page.getByRole("grid", { name: /rows by/ });
  await expect(grid.getByRole("row")).toHaveCount(8);
  await expect(grid.locator('[role="gridcell"][tabindex="0"]')).toHaveCount(1);
  await expectNoAccessibilityViolations(page);
});
