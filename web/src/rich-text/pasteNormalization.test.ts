import { describe, expect, it } from "vitest";
import { normalizeClipboardHtml, recoverMarkdownTableBreaks } from "./pasteNormalization";

describe("rich-text clipboard normalization", () => {
  it("wraps inline table-cell content without changing its text", () => {
    const html = normalizeClipboardHtml("<table><tr><td>First<br>second</td></tr></table>");
    const document = new DOMParser().parseFromString(html, "text/html");
    expect(document.querySelector("td > p")?.innerHTML).toBe("First<br>second");
  });

  it("recognizes task-list checkboxes without retaining interactive inputs", () => {
    const html = normalizeClipboardHtml(
      '<ul><li><input type="checkbox" checked>Done</li><li><input type="checkbox">Later</li></ul>'
    );
    const document = new DOMParser().parseFromString(html, "text/html");
    expect(document.querySelector("ul")?.getAttribute("data-type")).toBe("taskList");
    expect(
      [...document.querySelectorAll("li")].map((item) => item.getAttribute("data-checked"))
    ).toEqual(["true", "false"]);
    expect(document.querySelectorAll("input")).toHaveLength(0);
  });

  it("recovers table breaks only when Markdown corroborates flattened HTML", () => {
    const html =
      "<table><tr><th>Line</th><th>Other</th></tr><tr><td>firstsecond</td><td>value</td></tr></table>";
    const markdown = "| Line | Other |\n| --- | --- |\n| first<br>second | value |";
    const recovered = recoverMarkdownTableBreaks(html, markdown);
    expect(recovered).not.toBeNull();
    const document = new DOMParser().parseFromString(recovered!, "text/html");
    expect(document.querySelector("td")?.innerHTML).toBe("first<br>second");
    expect(
      recoverMarkdownTableBreaks(
        html,
        "| Line | Other |\n| --- | --- |\n| unrelated<br>text | value |"
      )
    ).toBeNull();
  });
});
