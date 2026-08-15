import { expect, test } from "@playwright/test";
import { mockApi, paste } from "./support/mockApi";

test("untouched paste form navigates without a discard prompt", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await expect(page.getByRole("heading", { name: "New paste" })).toBeVisible();
  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page).toHaveURL(/\/pastes$/);
  await expect(page.getByRole("heading", { name: "My pastes" })).toBeVisible();
});

test("switching an empty paste to rich text does not create unsaved content", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await expect(page.locator('.rich-text-editor[data-editor-ready="true"]')).toBeVisible();

  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page).toHaveURL(/\/pastes$/);
  await expect(page.getByRole("heading", { name: "Discard unsaved changes?" })).toHaveCount(0);
});

test("expiration presets populate a stable, customizable date control", async ({ page }) => {
  await mockApi(page, true);
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/pastes/new");

  const expiration = page.getByRole("combobox", { name: "Expiration" });
  const date = page.getByLabel("Date and time");
  const readLimit = page.getByLabel("View limit");
  await expect(expiration).toHaveValue("never");
  await expect(date).toBeDisabled();
  await expect(date).toHaveValue("Not applicable");

  await expiration.selectOption("1w");
  await expect(date).toBeEnabled();
  await expect(date).not.toHaveValue("");
  const [dateBox, readLimitBox] = await Promise.all([date.boundingBox(), readLimit.boundingBox()]);
  expect(dateBox!.x + dateBox!.width).toBeLessThan(readLimitBox!.x);

  await date.fill("2030-01-02T03:04");
  await expect(expiration).toHaveValue("custom");
  await expect(date).toHaveValue("2030-01-02T03:04");

  await expiration.selectOption("never");
  await expect(date).toBeDisabled();
  await expect(date).toHaveValue("Not applicable");
  await expiration.selectOption("custom");
  await expect(date).toBeEnabled();
  await expect(date).toHaveValue("");
});

test("plain text is uncolored by default", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const language = page.getByRole("combobox", { name: /Language/ });
  const editor = page.getByRole("textbox", { name: "Paste content" });
  await expect(language).toHaveValue("plaintext");
  await editor.fill("function greet(name) { console.log(`hello ${name}`); }");
  await expect(language).toHaveValue("plaintext");
  await expect(page.locator(".code-editor .hljs-keyword")).toHaveCount(0);
});

test("editing triggers the custom discard dialog and explicitly enabled auto detection detects JavaScript", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: /Language/ }).click();
  await page.getByRole("option", { name: /Auto detect/ }).click();
  const editor = page.getByRole("textbox", { name: "Paste content" });
  await editor.fill("function greet(name) { console.log(`hello ${name}`); }");
  await expect(page.getByRole("combobox", { name: /Language/ })).toHaveValue("javascript");
  await page.getByRole("link", { name: "My pastes" }).click();
  await expect(page.getByRole("heading", { name: "Discard unsaved changes?" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(page).toHaveURL(/\/pastes\/new$/);
});

test("code editor caret and highlighted text retain the same scroll viewport", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const editor = page.getByRole("textbox", { name: "Paste content" });
  const content = Array.from(
    { length: 90 },
    (_, index) => `${String(index + 1).padStart(3, "0")} ${"long line ".repeat(20)}`
  ).join("\n");
  await editor.fill(content);
  await editor.evaluate(element => {
    element.scrollTop = element.scrollHeight;
    element.scrollLeft = element.scrollWidth;
    element.dispatchEvent(new Event("scroll"));
  });
  await expect.poll(() => page.locator(".code-editor").evaluate(container => {
    const textarea = container.querySelector("textarea")!;
    const overlay = container.querySelector("pre")!;
    const gutter = container.querySelector<HTMLElement>(".line-numbers")!;
    return {
      heightsMatch: overlay.clientHeight === textarea.clientHeight
        && gutter.clientHeight === textarea.clientHeight,
      verticalScrollMatches: Math.abs(overlay.scrollTop - textarea.scrollTop) < 1
        && Math.abs(gutter.scrollTop - textarea.scrollTop) < 1,
      horizontalScrollMatches: Math.abs(overlay.scrollLeft - textarea.scrollLeft) < 1
    };
  })).toEqual({
    heightsMatch: true,
    verticalScrollMatches: true,
    horizontalScrollMatches: true
  });
});

test("resizing the text editor grows the complete editor and is retained across modes", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const editor = page.locator(".content-editor");
  await editor.evaluate(element => { (element as HTMLElement).style.height = "620px"; });
  await expect.poll(() => page.locator(".content-editor").evaluate(element =>
    element.getBoundingClientRect().height
  )).toBe(620);
  const layers = await page.locator(".code-editor").evaluate(container => ({
    editor: container.getBoundingClientRect().height,
    textarea: container.querySelector("textarea")!.getBoundingClientRect().height,
    overlay: container.querySelector("pre")!.getBoundingClientRect().height,
    gutter: container.querySelector(".line-numbers")!.getBoundingClientRect().height
  }));
  expect(layers.editor).toBe(620);
  expect(layers.textarea).toBe(618);
  expect(layers.overlay).toBe(layers.textarea);
  expect(layers.gutter).toBe(layers.textarea);
  await expect(page.getByRole("textbox", { name: "Paste content" })).toHaveCSS("resize", "none");

  await page.locator(".form-grid select").first().selectOption("markdown");
  await expect(page.locator(".rich-text-editor")).toBeVisible();
  await expect(page.locator(".content-editor")).toHaveCSS("height", "620px");
  await expect(page.locator(".content-editor")).toHaveCSS("resize", "vertical");

  await page.locator(".content-editor").evaluate(element => {
    (element as HTMLElement).style.height = "700px";
  });
  await expect(page.locator(".content-editor")).toHaveCSS("height", "700px");
  await page.locator(".form-grid select").first().selectOption("text");
  await expect(page.getByRole("textbox", { name: "Paste content" })).toBeVisible();
  await expect(page.locator(".content-editor")).toHaveCSS("height", "700px");
});

test("empty rich-text conversion skips preview and disables language", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const language = page.getByRole("combobox", { name: /Language/ });
  await language.click();
  await page.getByRole("option", { name: /JavaScript/ }).click();
  await expect(language).toHaveValue("javascript");
  const textEditorHeight = await page.locator(".content-editor").evaluate(
    element => element.getBoundingClientRect().height
  );
  const textControlsTop = await page.locator(".form-grid").evaluate(
    element => element.getBoundingClientRect().top
  );
  await page.locator(".form-grid select").first().selectOption("markdown");
  await expect(page.getByRole("heading", { name: /Convert to/ })).toHaveCount(0);
  await expect(language).toBeDisabled();
  await expect(language).toHaveValue("Not applicable");
  await expect(page.locator(".rich-text-editor")).toBeVisible();
  const richTextEditorHeight = await page.locator(".content-editor").evaluate(
    element => element.getBoundingClientRect().height
  );
  const richTextControlsTop = await page.locator(".form-grid").evaluate(
    element => element.getBoundingClientRect().top
  );
  expect(richTextEditorHeight).toBe(textEditorHeight);
  expect(richTextControlsTop).toBe(textControlsTop);
  await page.locator(".form-grid select").first().selectOption("text");
  await page.getByRole("button", { name: "Convert" }).click();
  await expect(language).toBeEnabled();
  await expect(language).toHaveValue("javascript");
});

test("paste form labels share the same dark-mode color", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const colors = await page.locator(".form-grid").evaluate(form => {
    const color = (element: Element | null) => getComputedStyle(element!).color;
    return {
      type: color(form.querySelector("label > span")),
      language: color(form.querySelector(".language-field > label")),
      folder: color([...form.querySelectorAll("label > span")]
        .find(label => label.textContent === "Folder") ?? null),
      visibility: color([...form.querySelectorAll("label > span")]
        .find(label => label.textContent === "Visibility") ?? null)
    };
  });
  expect(new Set(Object.values(colors)).size).toBe(1);
});

test("rich-text formatting uses a single-row icon toolbar and confirms clearing", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.locator(".form-grid select").first().selectOption("markdown");
  const toolbar = page.getByRole("toolbar", { name: "Rich-text formatting" });
  await expect(toolbar.getByRole("button")).toHaveCount(19);
  await expect(toolbar.getByRole("button", { name: "Paragraph" })).toHaveText("¶");
  await expect(toolbar.getByRole("button", { name: "Heading 1" })).toHaveText("H1");
  await expect(toolbar.getByRole("button", { name: "Bulleted list" }).locator("svg")).toBeVisible();
  const rows = await toolbar.getByRole("button").evaluateAll(buttons =>
    new Set(buttons.map(button => Math.round(button.getBoundingClientRect().top))).size
  );
  expect(rows).toBe(1);

  await page.getByLabel("Rich-text paste content").fill("Formatted text");
  const bold = toolbar.getByRole("button", { name: "Bold" });
  await bold.click();
  await expect(bold).toHaveAttribute("aria-pressed", "true");
  page.once("dialog", dialog => {
    expect(dialog.message()).toBe("Clear all formatting from this rich-text paste?");
    void dialog.dismiss();
  });
  await toolbar.getByRole("button", { name: "Clear all formatting" }).click();
});

test("ordered rich-text lists can be submitted", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.locator(".form-grid select").first().selectOption("markdown");
  await page.getByLabel("Rich-text paste content").fill("First item");
  await page.getByRole("button", { name: "Numbered list" }).click();

  const submitted = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/pastes") && request.method() === "POST"
  );
  await page.getByRole("button", { name: "Create paste" }).click();
  const body = (await submitted).postDataJSON();
  expect(body.body).toMatchObject({ format: "markdown" });
  expect(body.body.content).toContain("1. First item");
  expect(body).not.toHaveProperty("expires_at");
  expect(body).not.toHaveProperty("read_limit");
  expect(body).not.toHaveProperty("folder_id");
});

test("task lists use checkbox rows without ordinary list markers", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await page.getByLabel("Rich-text paste content").fill("Required task");
  await page.getByRole("button", { name: "Task list" }).click();

  const checkbox = page.getByRole("checkbox", { name: "Task item checkbox for Required task" });
  const task = checkbox.locator("xpath=ancestor::li");
  const taskList = task.locator("xpath=parent::ul");
  await expect(checkbox).toBeVisible();
  await expect(taskList).toHaveCSS("list-style-type", "none");
  await expect(task).toHaveCSS("display", "flex");
  const alignment = await task.evaluate(item => {
    const checkboxBox = item.querySelector("input")!.getBoundingClientRect();
    const paragraphBox = item.querySelector("p")!.getBoundingClientRect();
    return Math.abs((checkboxBox.top + checkboxBox.height / 2)
      - (paragraphBox.top + Number.parseFloat(getComputedStyle(item).lineHeight) / 2));
  });
  expect(alignment).toBeLessThan(2);
});

test("visual bullet lists use compact item spacing", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.fill("First item");
  await page.getByRole("button", { name: "Bulleted list" }).click();
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Second item");
  const items = page.locator(".rich-text-editor ul li");
  await expect(items).toHaveCount(2);
  const gap = await items.evaluateAll(elements => {
    const first = elements[0]!.getBoundingClientRect();
    const second = elements[1]!.getBoundingClientRect();
    return second.top - first.bottom;
  });
  expect(gap).toBeLessThan(8);
});

test("table picker inserts the selected size and exposes contextual editing controls", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await page.getByRole("button", { name: "Insert table" }).click();

  const picker = page.getByRole("dialog", { name: "Choose table size" });
  await expect(picker).toBeVisible();
  const placement = await page.getByRole("button", { name: "Insert table" }).evaluate(trigger => {
    const triggerBox = trigger.getBoundingClientRect();
    const toolbarBox = trigger.closest(".rich-text-toolbar")!.getBoundingClientRect();
    const pickerBox = document.querySelector<HTMLElement>(".table-picker")!.getBoundingClientRect();
    return {
      leftOffset: Math.abs(pickerBox.left - triggerBox.left),
      clearsToolbar: pickerBox.top >= toolbarBox.bottom,
      extendsBeyondToolbar: pickerBox.bottom > toolbarBox.bottom
    };
  });
  expect(placement.leftOffset).toBeLessThan(2);
  expect(placement.clearsToolbar).toBe(true);
  expect(placement.extendsBeyondToolbar).toBe(true);
  await expect(picker.getByRole("gridcell", { name: "1 row by 1 column" })).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowDown");
  await expect(picker.getByRole("gridcell", { name: "2 rows by 2 columns" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(picker).toBeHidden();
  await expect(page.getByRole("button", { name: "Insert table" })).toBeFocused();
  await page.getByRole("button", { name: "Insert table" }).click();
  await picker.getByRole("gridcell", { name: "4 rows by 5 columns" }).hover();
  await expect(picker.getByText("4 × 5 table")).toBeVisible();
  await picker.getByRole("gridcell", { name: "4 rows by 5 columns" }).click();

  const table = page.locator(".rich-text-editor table");
  await expect(table.locator("tr")).toHaveCount(4);
  await expect(table.locator("tr").first().locator("th, td")).toHaveCount(5);
  const controls = page.getByRole("group", { name: "Edit table" });
  await expect(controls).toBeVisible();
  await controls.getByRole("button", { name: "Add row below" }).click();
  await expect(table.locator("tr")).toHaveCount(5);
});

test("pasted links are normalized to the supported document contract", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.locator(".form-grid select").first().selectOption("markdown");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.focus();
  await editor.evaluate(element => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/plain", "Relative link and phone");
    clipboard.setData("text/html", '<p><a href="/help" target="_self" rel="external" class="button" onclick="alert(1)">Relative link</a> and <a href="tel:+15551212">phone</a></p>');
    element.dispatchEvent(new ClipboardEvent("paste", {
      bubbles: true, cancelable: true, clipboardData: clipboard
    }));
  });

  const submitted = page.waitForRequest(request =>
    request.url().endsWith("/api/v1/pastes") && request.method() === "POST"
  );
  await page.getByRole("button", { name: "Create paste" }).click();
  const body = (await submitted).postDataJSON();
  expect(body.body).toMatchObject({ format: "markdown" });
  expect(body.body.content).toContain("[Relative link](/help)");
  expect(body.body.content).not.toContain("onclick");
  expect(body.body.content).not.toContain("tel:");
});

test("pasted formatting cannot introduce non-GFM underline syntax", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.focus();
  await editor.evaluate(element => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/plain", "underlined text");
    clipboard.setData("text/html", "<p><u>underlined text</u></p>");
    element.dispatchEvent(new ClipboardEvent("paste", {
      bubbles: true, cancelable: true, clipboardData: clipboard
    }));
  });
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Paste content" })).toHaveValue("underlined text");
});

test("rich-text conversion populates the plain-text editor", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  const type = page.locator(".form-grid select").first();
  await type.selectOption("markdown");
  await page.getByLabel("Rich-text paste content").fill("Rich content");
  await type.selectOption("text");
  await expect(page.getByRole("heading", { name: "Convert to text?" })).toBeVisible();
  await expect(page.locator(".conversion-dialog pre")).toContainText(paste.content);
  await page.getByRole("button", { name: "Convert" }).click();
  await expect(page.locator(".code-editor textarea")).toHaveValue(paste.content);
});

test("rich text switches between visual editing and canonical Markdown source", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  const geometry = await page.locator(".rich-editor-pane").evaluate(pane => ({
    pane: pane.getBoundingClientRect().height,
    editor: pane.querySelector(".code-editor")!.getBoundingClientRect().height
  }));
  expect(geometry.pane).toBeGreaterThan(300);
  expect(geometry.editor).toBeCloseTo(geometry.pane, 0);
  await source.fill("## Scene\n\n- [x] Ready");
  await page.getByRole("button", { name: "Visual", exact: true }).click();
  await expect(page.getByLabel("Rich-text paste content")).toContainText("Scene");
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  await expect(source).toHaveValue("## Scene\n\n- [x] Ready");
});

test("the supported Markdown document contract survives a visual-editor round trip", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  const document = [
    "# Heading",
    "",
    "**bold** *italic* ~~strike~~ `inline` [relative](/help)",
    "",
    "> quoted",
    "",
    "- first",
    "  - nested",
    "- second",
    "",
    "3. third",
    "4. fourth",
    "",
    "- [x] complete",
    "- [ ] pending",
    "",
    "```javascript",
    "const answer = 42;",
    "```",
    "",
    "---",
    "",
    "| A | B |",
    "| :--- | ---: |",
    "| one | two |"
  ].join("\n");
  await source.fill(document);
  await page.getByRole("button", { name: "Visual", exact: true }).click();
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  await expect(source).toHaveValue(document);
});

test("code blocks survive visual and Markdown mode round trips", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await page.getByLabel("Rich-text paste content").fill("const answer = 42;");
  await page.getByRole("button", { name: "Code block" }).click();
  await expect(page.locator(".rich-text-editor pre code")).toContainText("const answer = 42;");

  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  await expect(source).toHaveValue(/```[\s\S]*const answer = 42;[\s\S]*```/);
  await page.getByRole("button", { name: "Visual", exact: true }).click();
  await expect(page.locator(".rich-text-editor pre code")).toContainText("const answer = 42;");
});

test("pasted code blocks do not acquire editable trailing blank lines", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.focus();
  await editor.evaluate(element => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/html", "<p>Example:</p><pre><code>const answer = 42;\n</code></pre>");
    clipboard.setData("text/plain", "Example:\n\nconst answer = 42;");
    element.dispatchEvent(new ClipboardEvent("paste", { clipboardData: clipboard, bubbles: true }));
  });
  await expect(page.locator(".rich-text-editor pre code")).toHaveText("const answer = 42;");
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  await expect.poll(async () => (await source.inputValue()).trimEnd())
    .toBe("Example:\n\n```\nconst answer = 42;\n```");
  expect(await source.inputValue()).not.toContain("const answer = 42;\n\n```");
});

test("pasted rich-text table cells preserve hard line breaks and formatting", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.focus();
  await editor.evaluate(element => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/html", `<table><tbody>
      <tr><th><span>Label</span></th><th><span>Details</span></th></tr>
      <tr><td><span>Example</span></td><td><strong><span>Bold line</span></strong><br><em><span>Italic line</span></em></td></tr>
    </tbody></table>`);
    clipboard.setData("text/plain", "Label\tDetails\nExample\tBold line\nItalic line");
    element.dispatchEvent(new ClipboardEvent("paste", { clipboardData: clipboard, bubbles: true }));
  });
  await expect(page.locator(".rich-text-editor td").last().locator("br")).toHaveCount(1);
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  await expect(source).toHaveValue(/\*\*Bold line\*\*<br>\*Italic line\*/);
  await page.getByRole("button", { name: "Visual", exact: true }).click();
  await expect(page.locator(".rich-text-editor td").last().locator("br")).toHaveCount(1);
});

test("pasted Markdown tables recover breaks omitted from clipboard HTML", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.focus();
  await editor.evaluate(element => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/html", `<table><thead><tr>
      <th align="left">Left aligned</th><th align="left">Multi-line cell</th>
    </tr></thead><tbody>
      <tr><td align="left">Alpha</td><td align="left">First lineSecond line</td></tr>
      <tr><td align="left">Beta</td><td align="left"><strong>Bold line</strong><em>Italic line</em></td></tr>
      <tr><td align="left">Gamma</td><td align="left"><a href="https://example.com">Link</a><code>code</code></td></tr>
    </tbody></table>`);
    clipboard.setData("text/plain", [
      "| Left aligned | Multi-line cell |",
      "| :----------- | :-------------- |",
      "| Alpha | First line<br>Second line |",
      "| Beta | **Bold line**<br>*Italic line* |",
      "| Gamma | [Link](https://example.com)<br>`code` |"
    ].join("\n"));
    element.dispatchEvent(new ClipboardEvent("paste", { clipboardData: clipboard, bubbles: true }));
  });
  await expect(page.locator(".rich-text-editor tbody td br")).toHaveCount(3);
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  await expect(source).toHaveValue(/First line<br>Second line/);
  await expect(source).toHaveValue(/\*\*Bold line\*\*<br>\*Italic line\*/);
  await expect(source).toHaveValue(/\[Link\]\(https:\/\/example\.com\)<br>`code`/);
});

test("flattened table HTML is unchanged without corroborating Markdown breaks", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.focus();
  await editor.evaluate(element => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/html", "<table><tbody><tr><td>First lineSecond line</td></tr></tbody></table>");
    clipboard.setData("text/plain", "First lineSecond line");
    element.dispatchEvent(new ClipboardEvent("paste", { clipboardData: clipboard, bubbles: true }));
  });
  await expect(page.locator(".rich-text-editor td br")).toHaveCount(0);
  await expect(page.locator(".rich-text-editor td")).toHaveText("First lineSecond line");
});

test("pasted semantic task lists become nested canonical task lists", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  const editor = page.getByLabel("Rich-text paste content");
  await editor.focus();
  await editor.evaluate(element => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/html", `<ul>
      <li data-task-list-item="true" data-checked="true"><span contenteditable="false"><input type="checkbox" checked></span><div><p>Completed task</p></div></li>
      <li data-task-list-item="true" data-checked="false"><span contenteditable="false"><input type="checkbox"></span><div><p>Parent task</p><ul><li data-task-list-item="true" data-checked="true"><span contenteditable="false"><input type="checkbox" checked></span><div><p>Nested task</p></div></li></ul></div></li>
    </ul>`);
    clipboard.setData("text/plain", "Completed task\nParent task\nNested task");
    element.dispatchEvent(new ClipboardEvent("paste", { clipboardData: clipboard, bubbles: true }));
  });
  await expect(page.locator('.rich-text-editor ul[data-type="taskList"]')).toHaveCount(2);
  await expect(page.locator('.rich-text-editor li[data-checked]')).toHaveCount(3);
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  await expect.poll(async () => (await source.inputValue()).trimEnd()).toBe([
    "- [x] Completed task",
    "- [ ] Parent task",
    "  - [x] Nested task"
  ].join("\n"));
});

test("table cells prevent block structures that canonical Markdown cannot preserve", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await page.getByRole("button", { name: "Insert table" }).click();
  await page.getByRole("dialog", { name: "Choose table size" })
    .getByRole("gridcell", { name: "1 row by 1 column" }).click();
  const cell = page.locator(".rich-text-editor th");
  await cell.click();
  await page.keyboard.type("zxcdsdsaf");
  await expect(page.getByRole("button", { name: "Code block" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Inline code" })).toBeEnabled();
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  const source = page.getByRole("textbox", { name: "Paste content" });
  await expect(source).toHaveValue(/zxcdsdsaf/);
  await page.getByRole("button", { name: "Visual", exact: true }).click();
  await expect(page.locator(".rich-text-editor th")).toContainText("zxcdsdsaf");
  await expect(page.locator(".rich-text-editor pre")).toHaveCount(0);
});

test("table cell line breaks remain valid canonical Markdown", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await page.getByRole("button", { name: "Insert table" }).click();
  await page.getByRole("dialog", { name: "Choose table size" })
    .getByRole("gridcell", { name: "1 row by 1 column" }).click();
  await page.locator(".rich-text-editor th").click();
  await page.keyboard.type("first");
  await page.keyboard.press("Shift+Enter");
  await page.keyboard.type("second");
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Paste content" })).toHaveValue(/first<br>second/i);
  await page.getByRole("button", { name: "Visual", exact: true }).click();
  await expect(page.locator(".rich-text-editor th br")).toHaveCount(1);
});

test("table row editing retains a Markdown-compatible header row", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/new");
  await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("markdown");
  await page.getByRole("button", { name: "Insert table" }).click();
  await page.getByRole("dialog", { name: "Choose table size" })
    .getByRole("gridcell", { name: "2 rows by 2 columns" }).click();
  await page.locator(".rich-text-editor th").first().click();
  await page.getByRole("group", { name: "Edit table" })
    .getByRole("button", { name: "Delete row" }).click();
  await page.locator(".rich-text-editor td").first().click();
  await page.keyboard.type("value");
  await page.getByRole("button", { name: "Markdown", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Paste content" })).toHaveValue(/value/);
  await page.getByRole("button", { name: "Visual", exact: true }).click();
  await expect(page.locator(".rich-text-editor table")).toContainText("value");
  await expect(page.locator(".rich-text-editor th")).toHaveCount(2);
});

test("attachment selections accumulate in a removable upload queue", async ({ page }) => {
  await mockApi(page, true);
  let multipart = "";
  page.on("request", request => {
    if (new URL(request.url()).pathname === "/api/v1/pastes" && request.method() === "POST") {
      multipart = request.postData() ?? "";
    }
  });
  await page.goto("/pastes/new");
  const picker = page.getByLabel("Add attachments");
  await picker.setInputFiles({
    name: "first.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("first")
  });
  await picker.setInputFiles({
    name: "second.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("second")
  });

  const queue = page.getByRole("region", { name: "Selected attachments" });
  await expect(queue.getByText("first.txt")).toBeVisible();
  await expect(queue.getByText("second.txt")).toBeVisible();
  await expect(queue).toContainText("2 files");
  await page.getByRole("button", { name: "Remove first.txt" }).click();
  await expect(queue.getByText("first.txt")).toBeHidden();

  await page.getByRole("button", { name: "Create paste" }).click();
  await expect(page).toHaveURL(/\/pastes\/sample-paste$/);
  expect(multipart).toContain("second.txt");
  expect(multipart).not.toContain("first.txt");
});

test("edit page shows current attachments", async ({ page }) => {
  await mockApi(page, true);
  await page.goto("/pastes/sample-paste/edit");
  await expect(page.getByText("Current attachments")).toBeVisible();
  await expect(page.getByRole("link", { name: /example.txt/ })).toBeVisible();
  await expect(page.getByText(/takes effect immediately/)).toBeVisible();
});

test("attachment deletion carries the returned revision into the next edit", async ({ page }) => {
  await mockApi(page, true);
  let deleteMatch = "";
  let patchMatch = "";
  await page.route("**/api/v1/pastes/sample-paste**", async route => {
    const request = route.request();
    const pathname = new URL(request.url()).pathname;
    if (pathname.endsWith("/source") && request.method() === "GET") {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { ETag: '"paste-sample-paste-1"' },
        body: JSON.stringify(paste)
      });
    }
    if (pathname.endsWith("/attachments/7") && request.method() === "DELETE") {
      deleteMatch = request.headers()["if-match"] ?? "";
      return route.fulfill({ status: 204, headers: { ETag: '"paste-sample-paste-2"' } });
    }
    if (pathname === "/api/v1/pastes/sample-paste" && request.method() === "PATCH") {
      patchMatch = request.headers()["if-match"] ?? "";
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { ETag: '"paste-sample-paste-3"' },
        body: JSON.stringify({ ...paste, attachments: [], attachment_count: 0 })
      });
    }
    return route.fallback();
  });

  await page.goto("/pastes/sample-paste/edit");
  page.once("dialog", dialog => dialog.accept());
  await page.getByRole("button", { name: "Delete example.txt" }).click();
  await expect(page.getByRole("link", { name: /example.txt/ })).toBeHidden();
  await page.getByLabel("Title").fill("Updated after attachment removal");
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect.poll(() => [deleteMatch, patchMatch]).toEqual([
    '"paste-sample-paste-1"',
    '"paste-sample-paste-2"'
  ]);
});

test("failed edit attachment upload preserves the saved revision and retry state", async ({ page }) => {
  await mockApi(page, true);
  const patchHeaders: string[] = [];
  await page.route("**/api/v1/pastes/sample-paste**", async route => {
    const request = route.request();
    const pathname = new URL(request.url()).pathname;
    if (pathname === "/api/v1/pastes/sample-paste" && request.method() === "PATCH") {
      patchHeaders.push(request.headers()["if-match"] ?? "");
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { ETag: '"paste-sample-paste-2"' },
        body: JSON.stringify({ ...paste, title: "Saved title" })
      });
    }
    if (pathname.endsWith("/attachments") && request.method() === "POST") {
      return route.fulfill({
        status: 422,
        contentType: "application/problem+json",
        body: JSON.stringify({ detail: "Attachment was rejected" })
      });
    }
    return route.fallback();
  });
  await page.goto("/pastes/sample-paste/edit");
  await page.getByLabel("Title").fill("Saved title");
  await page.getByLabel("Add attachments").setInputFiles({
    name: "retry.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("retry")
  });
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Paste changes were saved, but attachments were not uploaded"
  );
  await expect(page).toHaveURL(/\/pastes\/sample-paste\/edit$/);
  await page.getByRole("button", { name: "Save changes" }).click();
  await expect.poll(() => patchHeaders).toEqual(["*", "\"paste-sample-paste-2\""]);
});
