import { Editor } from "@tiptap/core";
import Link from "@tiptap/extension-link";
import TextAlign from "@tiptap/extension-text-align";
import Underline from "@tiptap/extension-underline";
import StarterKit from "@tiptap/starter-kit";

export type RichTextDocument = Record<string, unknown>;

const extensions = [
  StarterKit.configure({
    heading: { levels: [1, 2, 3] },
    codeBlock: {},
    dropcursor: false,
    gapcursor: false
  }),
  Underline,
  Link.configure({
    openOnClick: false,
    autolink: true,
    protocols: ["http", "https", "mailto"],
    HTMLAttributes: { rel: "noopener noreferrer nofollow", target: "_blank" }
  }),
  TextAlign.configure({ types: ["heading", "paragraph"], alignments: ["left", "center", "right"] })
];

let editableEditor: Editor | undefined;
let readonlyEditor: Editor | undefined;

export function mountRichTextEditor(
  element: HTMLElement,
  document: RichTextDocument,
  onChange?: () => void
): Editor {
  editableEditor?.destroy();
  editableEditor = new Editor({
    element,
    extensions,
    content: document,
    editorProps: {
      attributes: {
        class: "rich-text-content",
        "aria-label": "Rich-text paste content"
      },
      transformPastedHTML: html => html
        .replace(/<(script|style|iframe|object|embed)[\s\S]*?<\/\1>/gi, "")
        .replace(/\son\w+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)/gi, "")
    },
    onUpdate: () => onChange?.()
  });
  return editableEditor;
}

export function mountRichTextViewer(element: HTMLElement, document: RichTextDocument): void {
  readonlyEditor?.destroy();
  readonlyEditor = new Editor({
    element,
    extensions,
    content: document,
    editable: false,
    editorProps: {
      attributes: { class: "rich-text-content" }
    }
  });
}

export function richTextDocument(): RichTextDocument | undefined {
  return editableEditor?.getJSON() as RichTextDocument | undefined;
}

export function setRichTextDocument(document: RichTextDocument): void {
  editableEditor?.commands.setContent(document);
}

export function runRichTextCommand(command: string): void {
  if (!editableEditor) return;
  const chain = editableEditor.chain().focus();
  switch (command) {
    case "paragraph": chain.setParagraph().run(); break;
    case "heading-1": chain.toggleHeading({ level: 1 }).run(); break;
    case "heading-2": chain.toggleHeading({ level: 2 }).run(); break;
    case "heading-3": chain.toggleHeading({ level: 3 }).run(); break;
    case "bold": chain.toggleBold().run(); break;
    case "italic": chain.toggleItalic().run(); break;
    case "underline": chain.toggleUnderline().run(); break;
    case "strike": chain.toggleStrike().run(); break;
    case "code": chain.toggleCode().run(); break;
    case "bullet-list": chain.toggleBulletList().run(); break;
    case "ordered-list": chain.toggleOrderedList().run(); break;
    case "blockquote": chain.toggleBlockquote().run(); break;
    case "code-block": chain.toggleCodeBlock().run(); break;
    case "horizontal-rule": chain.setHorizontalRule().run(); break;
    case "align-left": chain.setTextAlign("left").run(); break;
    case "align-center": chain.setTextAlign("center").run(); break;
    case "align-right": chain.setTextAlign("right").run(); break;
    case "undo": chain.undo().run(); break;
    case "redo": chain.redo().run(); break;
    case "clear-formatting": chain.unsetAllMarks().clearNodes().run(); break;
    case "link": {
      const current = editableEditor.getAttributes("link").href as string | undefined;
      const href = prompt("Link URL", current ?? "https://");
      if (href === null) break;
      if (!href.trim()) chain.unsetLink().run();
      else chain.extendMarkRange("link").setLink({ href: href.trim() }).run();
      break;
    }
  }
}
