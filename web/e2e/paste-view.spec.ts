import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

test("paste view offers a print action and a paper-safe layout", async ({ page }) => {
  await page.addInitScript(() => {
    window.print = () => Object.assign(window, { __printed: true });
  });
  await mockApi(page, false);
  await page.goto("/pastes/sample-paste");
  await expect(page.getByRole("link", { name: "Raw" })).toHaveAttribute(
    "href",
    "/api/v1/pastes/sample-paste/raw"
  );
  await expect(page.locator(".paste-print-line")).toHaveCount(0);
  await page.getByRole("button", { name: "Print" }).click();
  await expect.poll(() => page.evaluate(() =>
    Boolean((window as Window & { __printed?: boolean }).__printed)
  )).toBe(true);

  await page.emulateMedia({ media: "print" });
  await expect(page.locator(".site-header")).toBeHidden();
  await expect(page.locator(".paste-view .page-heading .actions")).toBeHidden();
  await expect(page.locator(".paste-code-shell")).toBeHidden();
  await expect(page.locator(".paste-print-metadata")).toContainText(
    "Visibility: Unlisted · Language: JavaScript"
  );
  const printLayout = await page.locator(".paste-print-code").evaluate(viewer => {
    const lines = [...viewer.querySelectorAll<HTMLElement>(".paste-print-line")];
    const number = lines[0]!.querySelector<HTMLElement>("span")!;
    const content = lines[0]!.querySelector<HTMLElement>("code")!;
    return {
      displayed: getComputedStyle(viewer).display,
      lineCount: lines.length,
      contentWrap: getComputedStyle(content).whiteSpace,
      contentOverflowWrap: getComputedStyle(content).overflowWrap,
      keywordColor: getComputedStyle(content.querySelector(".hljs-keyword")!).color,
      plainColor: getComputedStyle(content).color,
      numberSelectable: getComputedStyle(number).userSelect
    };
  });
  expect(printLayout).toEqual({
    displayed: "block",
    lineCount: 2,
    contentWrap: "pre-wrap",
    contentOverflowWrap: "anywhere",
    keywordColor: "rgb(215, 58, 73)",
    plainColor: "rgb(36, 41, 46)",
    numberSelectable: "none"
  });
});

test("printed line-number gutter uses the widest number for every line", async ({ page }) => {
  const content = Array.from({ length: 100 }, (_, index) => `line ${index + 1}`).join("\n");
  await page.addInitScript(() => {
    window.print = () => undefined;
  });
  await mockApi(page, false, { viewPaste: {
    ...paste,
    content,
    body: { format: "text", content, language: "plaintext" },
    language: "plaintext"
  } });
  await page.goto("/pastes/sample-paste");
  await page.getByRole("button", { name: "Print" }).click();
  await page.emulateMedia({ media: "print" });

  const gutters = await page.locator(".paste-print-line").evaluateAll(lines =>
    [lines[0], lines[98], lines[99]].map(line => {
      const number = line!.querySelector<HTMLElement>("span")!;
      const content = line!.querySelector<HTMLElement>("code")!;
      return {
        gutterWidth: number.getBoundingClientRect().width,
        contentLeft: content.getBoundingClientRect().left
      };
    })
  );
  expect(gutters[0]!.gutterWidth).toBeCloseTo(gutters[2]!.gutterWidth, 5);
  expect(gutters[0]!.contentLeft).toBeCloseTo(gutters[1]!.contentLeft, 5);
  expect(gutters[0]!.contentLeft).toBeCloseTo(gutters[2]!.contentLeft, 5);
});

test("rich text keeps its document hierarchy in the shared print frame", async ({ page }) => {
  await mockApi(page, false, { viewPaste: {
    ...paste,
    content_kind: "markdown",
    format: "markdown",
    language: "plaintext",
    content: "## Section\n\nFormatted text",
    plain_text: "Section\n\nFormatted text",
    rendered_html: "<h2>Section</h2><p>Formatted text</p>"
  } });
  await page.goto("/pastes/sample-paste");
  await expect(page.locator(".rich-text-viewer")).toBeVisible();
  await page.emulateMedia({ media: "print" });
  await expect(page.locator(".paste-print-metadata")).toContainText(
    "Visibility: Unlisted · Format: Rich text"
  );
  const richLayout = await page.locator(".rich-text-viewer").evaluate(viewer => {
    const content = viewer.querySelector<HTMLElement>(".rich-text-content")!;
    const heading = content.querySelector<HTMLElement>("h2")!;
    const paragraph = content.querySelector<HTMLElement>("p")!;
    return {
      borderWidth: getComputedStyle(viewer).borderWidth,
      padding: getComputedStyle(content).padding,
      headingLarger: Number.parseFloat(getComputedStyle(heading).fontSize)
        > Number.parseFloat(getComputedStyle(paragraph).fontSize),
      alignment: getComputedStyle(paragraph).textAlign
    };
  });
  expect(richLayout).toEqual({
    borderWidth: "0px",
    padding: "0px",
    headingLarger: true,
    alignment: "start"
  });
});

test("Markdown pastes default to rendered output and expose canonical source", async ({ page }) => {
  await mockApi(page, false, { viewPaste: {
    ...paste, content_kind: "markdown", format: "markdown", language: "plaintext",
    content: "## Scene\n\n**Dialogue**", plain_text: "Scene\n\nDialogue",
    rendered_html: "<h2>Scene</h2><p><strong>Dialogue</strong></p>"
  } });
  await page.goto("/pastes/sample-paste");
  await expect(page.locator(".rich-text-viewer")).toContainText("Dialogue");
  const renderedControlTop = await page.getByRole("group", { name: "Paste representation" })
    .evaluate(element => element.getBoundingClientRect().top);
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  await expect(page.locator(".paste-code .content")).toContainText("## Scene");
  const markdownControlTop = await page.getByRole("group", { name: "Paste representation" })
    .evaluate(element => element.getBoundingClientRect().top);
  expect(markdownControlTop).toBe(renderedControlTop);
});

test("rendered task lists use aligned checkbox rows without ordinary markers", async ({ page }) => {
  await mockApi(page, false, { viewPaste: {
    ...paste, content_kind: "markdown", format: "markdown", language: "plaintext",
    content: "- [x] Complete\n- [ ] Pending", plain_text: "[x] Complete\n[ ] Pending",
    rendered_html: '<ul><li><input type="checkbox" checked disabled> Complete</li><li><input type="checkbox" disabled> Pending</li></ul>'
  } });
  await page.goto("/pastes/sample-paste");
  const list = page.locator(".rich-text-viewer ul");
  const item = list.locator("li").first();
  await expect(list).toHaveCSS("list-style-type", "none");
  await expect(item).toHaveCSS("display", "flex");
  const alignment = await item.evaluate(element => {
    const checkbox = element.querySelector("input")!.getBoundingClientRect();
    const lineHeight = Number.parseFloat(getComputedStyle(element).lineHeight);
    return Math.abs((checkbox.top + checkbox.height / 2) - (element.getBoundingClientRect().top + lineHeight / 2));
  });
  expect(alignment).toBeLessThan(2);
});

test("rendered Markdown tables retain declared column alignment", async ({ page }) => {
  await mockApi(page, false, { viewPaste: {
    ...paste, content_kind: "markdown", format: "markdown", language: "plaintext",
    content: "| Left | Center | Right |\n| :--- | :---: | ---: |\n| A | B | C |",
    plain_text: "Left\tCenter\tRight\nA\tB\tC",
    rendered_html: '<table><thead><tr><th align="left">Left</th><th align="center">Center</th><th align="right">Right</th></tr></thead></table>'
  } });
  await page.goto("/pastes/sample-paste");
  const headings = page.locator(".rich-text-viewer th");
  await expect(headings.nth(0)).toHaveCSS("text-align", "left");
  await expect(headings.nth(1)).toHaveCSS("text-align", "center");
  await expect(headings.nth(2)).toHaveCSS("text-align", "right");
});

test("Markdown representation controls remain fixed when the wrap option appears", async ({ page }) => {
  await mockApi(page, false, { viewPaste: {
    ...paste, content_kind: "markdown", format: "markdown", language: "plaintext",
    content: `## Scene\n\n${"wide content ".repeat(80)}`, plain_text: "Scene",
    rendered_html: "<h2>Scene</h2><p>Wide content</p>"
  } });
  await page.goto("/pastes/sample-paste");
  const controls = page.getByRole("group", { name: "Paste representation" });
  const renderedTop = await controls.evaluate(element => element.getBoundingClientRect().top);
  await controls.getByRole("button", { name: "Markdown" }).click();
  await expect(page.getByLabel("Wrap")).toBeVisible();
  expect(await controls.evaluate(element => element.getBoundingClientRect().top)).toBe(renderedTop);
  await controls.getByRole("button", { name: "Rendered" }).click();
  await expect(page.getByLabel("Wrap")).toBeHidden();
  expect(await controls.evaluate(element => element.getBoundingClientRect().top)).toBe(renderedTop);
});

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
