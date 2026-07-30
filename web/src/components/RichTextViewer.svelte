<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Editor } from "@tiptap/core";
  import TextAlign from "@tiptap/extension-text-align";
  import StarterKit from "@tiptap/starter-kit";
  import type { RichTextDocument } from "../types";

  let {
    document,
    onready
  }: {
    document: RichTextDocument;
    onready?: () => void;
  } = $props();
  let element: HTMLDivElement;

  onMount(() => {
    const editor = new Editor({
      element,
      extensions: [
        StarterKit.configure({
          heading: { levels: [1, 2, 3] },
          dropcursor: false,
          gapcursor: false,
          link: {
            openOnClick: true,
            protocols: ["http", "https", "mailto"],
            HTMLAttributes: { rel: "noopener noreferrer nofollow", target: "_blank" }
          }
        }),
        TextAlign.configure({ types: ["heading", "paragraph"], alignments: ["left", "center", "right"] })
      ],
      content: document,
      editable: false,
      editorProps: { attributes: { class: "rich-text-content" } }
    });
    void tick().then(onready);
    return () => editor.destroy();
  });
</script>

<div bind:this={element} class="rich-text-viewer"></div>
