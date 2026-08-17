import { expect, test } from "@playwright/test";
import { mockApi } from "./support/mockApi";

test.beforeEach(async ({ page }) => {
  await mockApi(page, true);
});

test("the bundled interface font is available", async ({ page }) => {
  await page.goto("/");
  const faces = await page.evaluate(async () =>
    (await document.fonts.load('16px "Racebin Inter"')).length
  );
  expect(faces).toBeGreaterThan(0);
});

test("desktop layout stays anchored without reserving an idle scrollbar gutter", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1400 });
  await page.goto("/pastes/new");
  const measure = () => page.evaluate(() => {
    const root = document.documentElement;
    const header = document.querySelector<HTMLElement>(".site-header")!;
    const heading = document.querySelector<HTMLElement>("h1")!;
    return {
      headingX: heading.getBoundingClientRect().x,
      headerWidth: header.getBoundingClientRect().width,
      rootWidth: root.getBoundingClientRect().width,
      viewportWidth: innerWidth,
      scrollbarGutter: getComputedStyle(root).scrollbarGutter,
      scrollable: root.scrollHeight > innerHeight
    };
  });
  const withoutScrollbar = await measure();
  expect(withoutScrollbar.scrollable).toBe(false);
  expect(withoutScrollbar.scrollbarGutter).toBe("auto");
  expect(withoutScrollbar.headerWidth).toBeCloseTo(withoutScrollbar.viewportWidth, 1);

  await page.evaluate(() => {
    const overflow = document.createElement("div");
    overflow.dataset.testOverflow = "true";
    overflow.style.height = "100vh";
    document.body.append(overflow);
  });
  await expect.poll(async () => (await measure()).scrollable).toBe(true);
  const withScrollbar = await measure();
  expect(withScrollbar.rootWidth).toBeLessThanOrEqual(withScrollbar.viewportWidth);
  expect(withScrollbar.headerWidth).toBeCloseTo(withScrollbar.viewportWidth, 1);
  expect(withScrollbar.headingX).toBeCloseTo(withoutScrollbar.headingX, 1);
});

test("primary pages do not overflow at supported widths", async ({ page }) => {
  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 390, height: 844 },
  ]) {
    await page.setViewportSize(viewport);
    for (const path of [
      "/pastes",
      "/explore",
      "/pastes/new",
      "/account",
      "/admin/pastes",
      "/admin/users",
      "/admin/users/1",
      "/help",
    ]) {
      await page.goto(path);
      await expect
        .poll(() =>
          page.locator("html").evaluate(
            (root) => root.scrollWidth <= root.clientWidth,
          ), { message: `${path} should fit at ${viewport.width}px` }
        )
        .toBe(true);
    }
  }
});

test("long paste identifiers do not displace mobile navigation", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/pastes/example-paste");
  const heading = page.getByRole("heading", { level: 1 });
  await heading.evaluate(element => {
    element.textContent = "f7f7113f74ab4a59baaac0ba";
  });

  await expect.poll(() => page.locator("html").evaluate(root =>
    root.scrollWidth <= root.clientWidth
  )).toBe(true);
  await expect.poll(() => page.locator(".primary-nav").evaluate(nav =>
    Math.abs(nav.getBoundingClientRect().bottom - window.innerHeight)
  )).toBeLessThan(1);
});

test("workspace sections share a common content edge", async ({ page }) => {
  await page.goto("/pastes");
  const edges = await page.locator(".paste-workspace-main").evaluate((main) => {
    const bounds = (selector: string) => {
      const rect = main.querySelector(selector)!.getBoundingClientRect();
      return { left: rect.left, right: rect.right };
    };
    return [
      bounds(".page-heading"),
      bounds(".paste-filter-form"),
      bounds(".paste-selection-bar"),
      bounds(".paste-list"),
    ];
  });
  for (const edge of edges.slice(1)) {
    expect(Math.abs(edge.left - edges[0].left)).toBeLessThan(1);
    expect(Math.abs(edge.right - edges[0].right)).toBeLessThan(1);
  }
});

test("page headings use consistent eyebrow-to-title spacing", async ({ page }) => {
  const headingGap = async () => page.locator(".page-heading").evaluate((heading) => {
    const eyebrow = heading.querySelector<HTMLElement>(".eyebrow")!.getBoundingClientRect();
    const title = heading.querySelector<HTMLElement>("h1")!.getBoundingClientRect();
    return title.top - eyebrow.bottom;
  });

  await page.goto("/pastes");
  const standardGap = await headingGap();
  await page.goto("/pastes/example-paste");

  expect(await headingGap()).toBeCloseTo(standardGap, 1);
});

test("page-level actions share one heading alignment", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  for (const route of [
    "/pastes",
    "/account",
    "/help",
    "/admin/users",
    "/admin/users/1",
    "/admin/invitations",
    "/admin/settings",
  ]) {
    await page.goto(route);
    const alignment = await page.locator(".page-heading").evaluate(heading => {
      const actions = heading.querySelector<HTMLElement>(".page-heading-actions");
      if (!actions) throw new Error("Page heading has no standard action container");
      const headingBox = heading.getBoundingClientRect();
      const actionBox = actions.getBoundingClientRect();
      return Math.abs(
        (headingBox.top + headingBox.height / 2) -
        (actionBox.top + actionBox.height / 2)
      );
    });
    expect(alignment, `${route} should center its page action`).toBeLessThan(1);
  }
});

test("primary pages share one heading-to-content boundary", async ({ page }) => {
  const routes = [
    "/pastes",
    "/explore",
    "/pastes/new",
    "/pastes/sample-paste",
    "/account",
    "/help",
    "/admin",
    "/admin/pastes",
    "/admin/users",
    "/admin/users/1",
    "/admin/invitations",
    "/admin/api-keys",
    "/admin/settings",
    "/admin/audit",
  ];

  let headingTop: number | undefined;
  for (const route of routes) {
    await page.goto(route);
    const heading = page.locator(".page-heading");
    await expect(heading, `${route} should render its page heading`).toBeVisible();
    const geometry = await heading.evaluate(element => {
      const content = element.nextElementSibling;
      if (!(content instanceof HTMLElement)) throw new Error("Page heading has no content sibling");
      const headingBox = element.getBoundingClientRect();
      const contentBox = content.getBoundingClientRect();
      return {
        parentClass: element.parentElement?.className,
        top: headingBox.top,
        gap: contentBox.top - headingBox.bottom,
      };
    });
    expect(String(geometry.parentClass), `${route} should use the shared page layout`).toContain("page-layout");
    headingTop ??= geometry.top;
    expect(geometry.top, `${route} heading top`).toBeCloseTo(headingTop, 1);
    expect(geometry.gap, `${route} heading boundary`).toBeCloseTo(20, 1);
  }

  await page.goto("/pastes");
  await expect(page.locator(".paste-filter-form")).toHaveCSS("border-top-style", "none");
});

test("filter expansion preserves the search toolbar boundary", async ({
  page,
}) => {
  await page.goto("/pastes");
  const toolbar = page.locator(".paste-filter-toolbar");
  const before = await toolbar.evaluate(
    (element) => element.getBoundingClientRect().bottom,
  );
  await page.getByRole("button", { name: /^Filters/ }).click();
  const after = await toolbar.evaluate(
    (element) => element.getBoundingClientRect().bottom,
  );
  expect(after).toBe(before);
  await expect(toolbar).toHaveCSS("border-bottom-style", "solid");
});

test("standard form controls use the shared control height", async ({
  page,
}) => {
  await page.goto("/pastes/new");
  const heights = await page
    .locator(
      '.form-grid input:not([type="checkbox"]):not([type="radio"]):not([type="file"]), .form-grid select',
    )
    .evaluateAll((elements) =>
      elements.map((element) => element.getBoundingClientRect().height),
    );
  expect(new Set(heights)).toEqual(new Set([40]));
  const controlTops = await page.locator(".type-field > select, .language-picker > input").evaluateAll(
    elements => elements.map(element => element.getBoundingClientRect().top),
  );
  expect(controlTops[0]).toBe(controlTops[1]);
});

test("checkbox rows retain native control geometry", async ({ page }) => {
  await mockApi(page, false);
  await page.goto("/login");
  const remember = page.getByRole("checkbox", { name: "Keep me signed in" });
  await expect(remember).toBeVisible();
  const geometry = await remember.evaluate(input => {
    const control = input.getBoundingClientRect();
    const label = input.closest("label")!;
    return {
      display: getComputedStyle(label).display,
      width: control.width,
      height: control.height,
    };
  });
  expect(geometry.display).toBe("flex");
  expect(geometry.width).toBeLessThanOrEqual(20);
  expect(geometry.height).toBeLessThanOrEqual(20);
});

test("form controls and composite editors share one complete focus ring", async ({ page }) => {
  await page.goto("/pastes/new");

  const focusRing = async (selector: string) => page.locator(selector).evaluate(element => {
    const style = getComputedStyle(element);
    return { color: style.outlineColor, style: style.outlineStyle, width: style.outlineWidth };
  });
  const expectedFocusColor = async () => page.evaluate(() => {
    const probe = document.createElement("span");
    probe.style.color = "var(--color-focus-ring)";
    document.body.append(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  });

  await page.getByLabel("Title").focus();
  const inputRing = await focusRing(".title-field input");

  await page.getByRole("textbox", { name: "Paste content" }).focus();
  const textEditorRing = await focusRing(".content-editor-text");
  await expect(page.locator(".content-editor-text textarea")).toHaveCSS("outline-style", "none");

  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const richContent = page.locator('.rich-text-editor[data-editor-ready="true"] .rich-text-content');
  await expect(richContent).toBeVisible();
  await richContent.focus();
  const richEditorRing = await focusRing(".content-editor-rich");
  await expect(richContent).toHaveCSS("outline-style", "none");

  expect(inputRing).toEqual({ color: await expectedFocusColor(), style: "solid", width: "2px" });
  expect(textEditorRing).toEqual(inputRing);
  expect(richEditorRing).toEqual(inputRing);

  await page.evaluate(() => { document.documentElement.dataset.colorScheme = "dark"; });
  await page.getByLabel("Title").focus();
  const darkRing = await focusRing(".title-field input");
  expect(darkRing).toEqual({ color: await expectedFocusColor(), style: "solid", width: "2px" });
  expect(darkRing.color).not.toBe(inputRing.color);
});

test("paste editor uses the page width without stretching metadata controls", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/pastes/new");

  const geometry = await page.locator("main, .editor, .form-grid > *").evaluateAll((elements, mainSelector) =>
    elements.map(element => {
      const box = element.getBoundingClientRect();
      const style = element.matches(mainSelector) ? getComputedStyle(element) : null;
      return {
        left: box.left,
        right: box.right,
        width: box.width,
        paddingLeft: style ? Number.parseFloat(style.paddingLeft) : 0,
        paddingRight: style ? Number.parseFloat(style.paddingRight) : 0
      };
    }), "main"
  );
  const [main, editor, ...controls] = geometry;

  expect(editor.left).toBe(main.left + main.paddingLeft);
  expect(editor.right).toBe(main.right - main.paddingRight);
  expect(controls.slice(0, 4).map(control => control.width)).toEqual([140, 260, 200, 140]);
  expect(controls[4]?.left).toBe(controls[0]?.left);
  expect(controls[4]?.width).toBe(140);
  expect(controls[5]?.left).toBe(controls[1]?.left);
  expect(controls[6]?.left).toBe(controls[2]?.left);
  expect(controls[6]?.width).toBe(120);
  expect(controls[1]!.left - controls[0]!.right).toBe(12);
  expect(controls[5]!.left - controls[4]!.right).toBe(12);
  const readLimitWidths = await page.locator(".read-limit-field").evaluate(field => ({
    field: field.getBoundingClientRect().width,
    input: field.querySelector("input")!.getBoundingClientRect().width
  }));
  expect(readLimitWidths).toEqual({ field: 120, input: 120 });
});
