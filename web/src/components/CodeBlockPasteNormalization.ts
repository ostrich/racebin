import { Extension } from "@tiptap/core";
import { Fragment, type Node as ProseMirrorNode, Slice } from "@tiptap/pm/model";
import { Plugin } from "@tiptap/pm/state";

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
export const CodeBlockPasteNormalization = Extension.create({
  name: "codeBlockPasteNormalization",
  addProseMirrorPlugins() {
    return [new Plugin({
      props: {
        transformPasted: slice => new Slice(
          Fragment.fromArray(slice.content.content.map(normalizeNode)),
          slice.openStart,
          slice.openEnd
        )
      }
    })];
  }
});
