import { Extension } from "@tiptap/core";
import { Fragment, type Node as ProseMirrorNode, Slice } from "@tiptap/pm/model";
import { Plugin } from "@tiptap/pm/state";

const blockElementNames = new Set([
  "ADDRESS", "ARTICLE", "ASIDE", "BLOCKQUOTE", "DIV", "DL", "FIELDSET", "FIGURE",
  "FOOTER", "FORM", "H1", "H2", "H3", "H4", "H5", "H6", "HEADER", "HR", "MAIN",
  "NAV", "OL", "P", "PRE", "SECTION", "TABLE", "UL"
]);

function normalizeTableCells(document: Document): void {
  for (const cell of document.querySelectorAll("td, th")) {
    const hasBlockContent = [...cell.children]
      .some(child => blockElementNames.has(child.tagName));
    if (hasBlockContent || !cell.childNodes.length) continue;

    const paragraph = document.createElement("p");
    paragraph.append(...cell.childNodes);
    cell.append(paragraph);
  }
}

function normalizeTaskLists(document: Document): void {
  const taskItems = document.querySelectorAll<HTMLElement>(
    "li[data-task-list-item], li:has(> input[type=checkbox]), li:has(> :not(ul, ol) > input[type=checkbox])"
  );
  for (const item of taskItems) {
    const checkbox = item.querySelector<HTMLInputElement>("input[type=checkbox]");
    const checked = item.dataset.checked === "true" || checkbox?.checked === true;
    item.dataset.type = "taskItem";
    item.dataset.checked = String(checked);

    const list = item.parentElement;
    if (list?.tagName === "UL") list.dataset.type = "taskList";

    checkbox?.closest("label, [contenteditable=false], .task-list-item-checkbox")?.remove();
    if (checkbox?.isConnected) checkbox.remove();
  }
}

function normalizeClipboardHtml(html: string): string {
  const document = new DOMParser().parseFromString(html, "text/html");
  normalizeTableCells(document);
  normalizeTaskLists(document);
  return document.body.innerHTML;
}

function splitMarkdownTableRow(line: string): string[] | null {
  const trimmed = line.trim();
  if (!trimmed.includes("|")) return null;

  const cells: string[] = [];
  let cell = "";
  let escaped = false;
  let codeDelimiterLength = 0;
  for (let index = 0; index < trimmed.length; index += 1) {
    const character = trimmed[index];
    if (escaped) {
      cell += character;
      escaped = false;
      continue;
    }
    if (character === "\\") {
      cell += character;
      escaped = true;
      continue;
    }
    if (character === "`") {
      let end = index;
      while (trimmed[end] === "`") end += 1;
      const length = end - index;
      if (codeDelimiterLength === 0) codeDelimiterLength = length;
      else if (codeDelimiterLength === length) codeDelimiterLength = 0;
      cell += trimmed.slice(index, end);
      index = end - 1;
      continue;
    }
    if (character === "|" && codeDelimiterLength === 0) {
      cells.push(cell.trim());
      cell = "";
      continue;
    }
    cell += character;
  }
  cells.push(cell.trim());
  if (trimmed.startsWith("|")) cells.shift();
  if (trimmed.endsWith("|") && !trimmed.endsWith("\\|")) cells.pop();
  return cells.length > 1 ? cells : null;
}

function markdownTables(text: string): string[][][] {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const tables: string[][][] = [];
  for (let index = 0; index + 1 < lines.length; index += 1) {
    const header = splitMarkdownTableRow(lines[index]!);
    const delimiter = splitMarkdownTableRow(lines[index + 1]!);
    if (!header || !delimiter || header.length !== delimiter.length
      || !delimiter.every(cell => /^:?-{3,}:?$/.test(cell))) continue;

    const rows = [header];
    index += 2;
    while (index < lines.length) {
      const row = splitMarkdownTableRow(lines[index]!);
      if (!row || row.length !== header.length) break;
      rows.push(row);
      index += 1;
    }
    tables.push(rows);
    index -= 1;
  }
  return tables;
}

function markdownInlineText(markdown: string, document: Document): string {
  const withoutMarkup = markdown
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/(`+)(.*?)\1/g, "$2")
    .replace(/(\*\*|__|~~|\*|_)/g, "")
    .replace(/\\([\\`*{}\[\]()#+\-.!_|>])/g, "$1")
    .replace(/<[^>]+>/g, "");
  const textarea = document.createElement("textarea");
  textarea.innerHTML = withoutMarkup;
  return textarea.value;
}

function insertBreakAtTextOffset(cell: Element, offset: number, document: Document): boolean {
  const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT);
  let consumed = 0;
  for (let node = walker.nextNode() as Text | null; node; node = walker.nextNode() as Text | null) {
    const end = consumed + node.data.length;
    if (offset > end) {
      consumed = end;
      continue;
    }

    const localOffset = offset - consumed;
    const br = document.createElement("br");
    if (localOffset > 0 && localOffset < node.data.length) {
      node.parentNode?.insertBefore(br, node.splitText(localOffset));
      return true;
    }

    let boundary: Node = node;
    while (boundary.parentNode && boundary.parentNode !== cell) boundary = boundary.parentNode;
    if (localOffset === 0) cell.insertBefore(br, boundary);
    else cell.insertBefore(br, boundary.nextSibling);
    return true;
  }
  return false;
}

function recoverMarkdownTableBreaks(html: string, text: string): string | null {
  const sourceTables = markdownTables(text);
  if (!sourceTables.length) return null;

  const document = new DOMParser().parseFromString(html, "text/html");
  const htmlTables = [...document.querySelectorAll("table")];
  if (htmlTables.length !== sourceTables.length) return null;
  let changed = false;

  for (let tableIndex = 0; tableIndex < htmlTables.length; tableIndex += 1) {
    const cells = [...htmlTables[tableIndex]!.querySelectorAll("th, td")];
    const sourceCells = sourceTables[tableIndex]!.flat();
    if (cells.length !== sourceCells.length) return null;

    for (let cellIndex = 0; cellIndex < cells.length; cellIndex += 1) {
      const cell = cells[cellIndex]!;
      const segments = sourceCells[cellIndex]!.split(/<br\s*\/?>/i);
      if (segments.length < 2 || cell.querySelector("br")) continue;
      const visibleSegments = segments.map(segment => markdownInlineText(segment, document));
      if (visibleSegments.join("") !== cell.textContent) continue;

      const offsets = visibleSegments.slice(0, -1).map((_, index) =>
        visibleSegments.slice(0, index + 1).join("").length
      );
      for (const offset of offsets.reverse()) {
        if (!insertBreakAtTextOffset(cell, offset, document)) return null;
      }
      changed = true;
    }
  }
  return changed ? document.body.innerHTML : null;
}

function normalizeNode(node: ProseMirrorNode): ProseMirrorNode {
  if (node.type.name === "codeBlock" && node.textContent.endsWith("\n")) {
    const content = node.textContent.slice(0, -1);
    return node.type.create(
      node.attrs,
      content ? node.type.schema.text(content) : undefined,
      node.marks
    );
  }
  if (!node.content.size) return node;
  return node.copy(Fragment.fromArray(node.content.content.map(normalizeNode)));
}

// Clipboard HTML commonly terminates <pre><code> content with a structural
// newline. It is not an editable blank line: Markdown fencing supplies that
// separator itself. Normalize pasted slices only, leaving typed and Markdown
// source whitespace exactly as the author entered it.
export const RichTextPasteNormalization = Extension.create({
  name: "richTextPasteNormalization",
  addProseMirrorPlugins() {
    let replayingNormalizedPaste = false;
    return [new Plugin({
      props: {
        handlePaste: (view, event) => {
          if (replayingNormalizedPaste) return false;
          const html = event.clipboardData?.getData("text/html");
          const text = event.clipboardData?.getData("text/plain");
          if (!html || !text) return false;
          const recovered = recoverMarkdownTableBreaks(html, text);
          if (!recovered) return false;
          replayingNormalizedPaste = true;
          try {
            return view.pasteHTML(recovered);
          } finally {
            replayingNormalizedPaste = false;
          }
        },
        transformPastedHTML: normalizeClipboardHtml,
        transformPasted: slice => new Slice(
          Fragment.fromArray(slice.content.content.map(normalizeNode)),
          slice.openStart,
          slice.openEnd
        )
      }
    })];
  }
});
