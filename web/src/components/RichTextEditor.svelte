<script lang="ts">
  import { onMount } from "svelte";
  import { Editor } from "@tiptap/core";
  import TextAlign from "@tiptap/extension-text-align";
  import StarterKit from "@tiptap/starter-kit";
  import Icon from "./Icon.svelte";
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

  const commands: Array<{
    command: string;
    label: string;
    icon?: string;
    symbol?: string;
    symbolClass?: string;
  }> = [
    { command: "paragraph", label: "Paragraph", symbol: "¶", symbolClass: "paragraph" },
    { command: "heading-1", label: "Heading 1", symbol: "H1" },
    { command: "heading-2", label: "Heading 2", symbol: "H2" },
    { command: "heading-3", label: "Heading 3", symbol: "H3" },
    { command: "bold", label: "Bold", symbol: "B", symbolClass: "bold" },
    { command: "italic", label: "Italic", symbol: "I", symbolClass: "italic" },
    { command: "underline", label: "Underline", symbol: "U", symbolClass: "underline" },
    { command: "strike", label: "Strikethrough", symbol: "S", symbolClass: "strike" },
    { command: "link", label: "Link", icon: "link" },
    { command: "bullet-list", label: "Bulleted list", icon: "list" },
    { command: "ordered-list", label: "Numbered list", icon: "list-ordered" },
    { command: "blockquote", label: "Block quote", icon: "quote" },
    { command: "code", label: "Inline code", icon: "code" },
    { command: "code-block", label: "Code block", icon: "square-code" },
    { command: "horizontal-rule", label: "Horizontal rule", icon: "minus" },
    { command: "align-left", label: "Align left", icon: "align-left" },
    { command: "align-center", label: "Align center", icon: "align-center" },
    { command: "align-right", label: "Align right", icon: "align-right" },
    { command: "clear-formatting", label: "Clear all formatting", icon: "eraser" },
    { command: "undo", label: "Undo", icon: "undo-2" },
    { command: "redo", label: "Redo", icon: "redo-2" }
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
      case "clear-formatting":
        if (confirm("Clear all formatting from this rich-text paste?")) {
          chain.selectAll().unsetAllMarks().clearNodes().run();
        }
        break;
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
  {#each commands as item}
    <button type="button" title={item.label} aria-label={item.label}
      onclick={() => run(item.command)}>
      {#if item.icon}
        <Icon name={item.icon}/>
      {:else}
        <span class:paragraph={item.symbolClass === "paragraph"}
          class:bold={item.symbolClass === "bold"}
          class:italic={item.symbolClass === "italic"}
          class:underline={item.symbolClass === "underline"}
          class:strike={item.symbolClass === "strike"}
          aria-hidden="true">{item.symbol}</span>
      {/if}
    </button>
  {/each}
</div>
<div bind:this={element} class="rich-text-editor"></div>
