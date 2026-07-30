<script lang="ts">
  import { highlightedCode, normalizeLanguage } from "../highlighting";

  let {
    value = $bindable(),
    language = $bindable()
  }: {
    value: string;
    language: string;
  } = $props();

  let textarea: HTMLTextAreaElement;
  let pre: HTMLPreElement;
  let gutter: HTMLDivElement;
  let html = $state("");
  let revision = 0;
  let lineCount = $derived(value.split("\n").length);
  let lineNumbers = $derived(Array.from({ length: lineCount }, (_, index) => index + 1).join("\n"));
  let width = $derived(`${Math.max(4, String(lineCount).length + 2)}ch`);

  async function render(code: string, syntax: string): Promise<void> {
    const current = ++revision;
    const requested = normalizeLanguage(syntax);
    const result = await highlightedCode(code, syntax);
    if (current !== revision) return;
    html = `${result.html}\n`;
    if (requested === "auto" && result.language) language = result.language;
  }

  function syncScroll(): void {
    if (!textarea || !pre || !gutter) return;
    pre.scrollTop = textarea.scrollTop;
    pre.scrollLeft = textarea.scrollLeft;
    gutter.scrollTop = textarea.scrollTop;
  }

  $effect(() => { void render(value, language); });
</script>

<div class="code-editor" style={`--line-number-width:${width}`}>
  <div bind:this={gutter} class="line-numbers" aria-hidden="true">{lineNumbers}</div>
  <pre bind:this={pre} aria-hidden="true"><code class="hljs">{@html html}</code></pre>
  <textarea bind:this={textarea} bind:value spellcheck="false" aria-label="Paste content"
    onscroll={syncScroll}></textarea>
</div>
