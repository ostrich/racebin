import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

test("wide paste offers synchronized sticky scrolling and aligned wrapped lines", async ({ page }) => {
  const content = Array.from(
    { length: 60 },
    (_, index) => `${String(index + 1).padStart(3, "0")} ${"wide content ".repeat(24)}`
  ).join("\n");
  await mockApi(page, false, { viewPaste: { ...paste, content } });
  await page.goto("/pastes/sample-paste");
  const code = page.locator(".paste-code-content-scroll");
  await expect.poll(() => code.evaluate(
    element => element.scrollWidth > element.clientWidth
  )).toBe(true);
  await page.evaluate(() => window.scrollTo(0, 500));
  const floating = page.getByRole("region", { name: "Horizontal paste scrollbar" });
  await expect(floating).toHaveClass(/visible/);
  const scrollbarAlignment = await page.locator(".paste-code-shell").evaluate(shell => {
    const gutteredViewer = shell.querySelector(".paste-code")!.getBoundingClientRect();
    const content = shell.querySelector(".paste-code-content-scroll")!.getBoundingClientRect();
    const stickyScrollbar = shell.querySelector(".paste-floating-scrollbar")!.getBoundingClientRect();
    return {
      startsAfterGutter: content.left > gutteredViewer.left,
      alignedWithContent: Math.abs(stickyScrollbar.left - content.left) < 1
    };
  });
  expect(scrollbarAlignment).toEqual({
    startsAfterGutter: true,
    alignedWithContent: true
  });
  const wrapTogglePosition = await page.getByRole("checkbox", { name: "Wrap" }).evaluate(input => {
    const toggle = input.closest("label")!.getBoundingClientRect();
    const heading = document.querySelector(".paste-view .page-heading")!.getBoundingClientRect();
    const viewer = document.querySelector(".paste-code-shell")!.getBoundingClientRect();
    return toggle.top >= heading.bottom && toggle.bottom <= viewer.top;
  });
  expect(wrapTogglePosition).toBe(true);
  await floating.evaluate(element => {
    element.scrollLeft = 240;
    element.dispatchEvent(new Event("scroll"));
  });
  await expect.poll(() => code.evaluate(element => element.scrollLeft)).toBeGreaterThan(200);

  const unwrappedFirstNumber = await page.locator(".line-numbers").evaluate(gutter => {
    const range = document.createRange();
    range.setStart(gutter.firstChild!, 0);
    range.setEnd(gutter.firstChild!, 1);
    const bounds = range.getBoundingClientRect();
    return { left: bounds.left + window.scrollX, top: bounds.top + window.scrollY };
  });
  await page.getByRole("checkbox", { name: "Wrap" }).check();
  await expect(page.locator(".paste-floating-scrollbar")).not.toHaveClass(/visible/);
  await expect.poll(() => code.evaluate(
    element => element.scrollWidth <= element.clientWidth + 1
  )).toBe(true);
  const lineLayout = await page.locator(".line-numbers.wrapped").evaluate(gutter => {
    const numbers = [...gutter.querySelectorAll<HTMLElement>("span")];
    const content = document.querySelector<HTMLElement>(".paste-code .content")!;
    const firstNumberRange = document.createRange();
    firstNumberRange.selectNodeContents(numbers[0]);
    const firstNumberBounds = firstNumberRange.getBoundingClientRect();
    return {
      count: numbers.length,
      firstGap: numbers[1].offsetTop - numbers[0].offsetTop,
      firstNumber: {
        left: firstNumberBounds.left + window.scrollX,
        top: firstNumberBounds.top + window.scrollY
      },
      firstLineAligned: Math.abs(
        numbers[0].getBoundingClientRect().top -
        (content.getBoundingClientRect().top + Number.parseFloat(getComputedStyle(content).paddingTop))
      ) < 1
    };
  });
  expect(lineLayout.count).toBe(60);
  expect(lineLayout.firstGap).toBeGreaterThan(22);
  expect(lineLayout.firstLineAligned).toBe(true);
  expect(Math.abs(lineLayout.firstNumber.left - unwrappedFirstNumber.left)).toBeLessThan(1);
  expect(Math.abs(lineLayout.firstNumber.top - unwrappedFirstNumber.top)).toBeLessThan(1);
});
