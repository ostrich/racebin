<script lang="ts">
  import { onMount } from "svelte";
  import { Editor } from "@tiptap/core";
  import TextAlign from "@tiptap/extension-text-align";
  import StarterKit from "@tiptap/starter-kit";
  import type { RichTextDocument } from "../types";

  let {
    document = $bindable(),
    onchange
  }: {
    document: RichTextDocument;
    onchange?: () => void;
  } = $props();
  let element: HTMLDivElement;
  let editor: Editor;

  const commands: Array<[string, string]> = [
    ["bold", "Bold"], ["italic", "Italic"], ["underline", "Underline"], ["strike", "Strike"],
    ["link", "Link"], ["bullet-list", "Bulleted list"], ["ordered-list", "Numbered list"],
    ["blockquote", "Quote"], ["code", "Inline code"], ["code-block", "Code block"],
    ["horizontal-rule", "Separator"], ["align-left", "Align left"], ["align-center", "Align center"],
    ["align-right", "Align right"], ["clear-formatting", "Clear formatting"],
    ["undo", "Undo"], ["redo", "Redo"]
  ];

  function run(command: string): void {
    const chain = editor.chain().focus();
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
        const current = editor.getAttributes("link").href as string | undefined;
        const href = prompt("Link URL", current ?? "https://");
        if (href === null) break;
        if (!href.trim()) chain.unsetLink().run();
        else chain.extendMarkRange("link").setLink({ href: href.trim() }).run();
        break;
      }
    }
  }

  onMount(() => {
    editor = new Editor({
      element,
      extensions: [
        StarterKit.configure({
          heading: { levels: [1, 2, 3] },
          codeBlock: {},
          dropcursor: false,
          gapcursor: false,
          link: {
            openOnClick: false,
            autolink: true,
            protocols: ["http", "https", "mailto"],
            HTMLAttributes: { rel: "noopener noreferrer nofollow", target: "_blank" }
          }
        }),
        TextAlign.configure({ types: ["heading", "paragraph"], alignments: ["left", "center", "right"] })
      ],
      content: document,
      editorProps: {
        attributes: { class: "rich-text-content", "aria-label": "Rich-text paste content" },
        transformPastedHTML: html => html
          .replace(/<(script|style|iframe|object|embed)[\s\S]*?<\/\1>/gi, "")
          .replace(/\son\w+\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)/gi, "")
      },
      onUpdate: ({ editor: updated }) => {
        document = updated.getJSON() as RichTextDocument;
        onchange?.();
      }
    });
    return () => editor.destroy();
  });
</script>

<div class="rich-text-toolbar" role="toolbar" aria-label="Rich-text formatting">
  <select aria-label="Block type" onchange={(event) => run(event.currentTarget.value)}>
    <option value="paragraph">Paragraph</option><option value="heading-1">Heading 1</option>
    <option value="heading-2">Heading 2</option><option value="heading-3">Heading 3</option>
  </select>
  {#each commands as [command, label]}
    <button type="button" title={label} aria-label={label} onclick={() => run(command)}>{label}</button>
  {/each}
</div>
<div bind:this={element} class="rich-text-editor"></div>
