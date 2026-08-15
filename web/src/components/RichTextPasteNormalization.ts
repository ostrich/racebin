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
    return [new Plugin({
      props: {
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
