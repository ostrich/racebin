<script lang="ts">
  import { tick } from "svelte";
  import { highlightedCode } from "../highlighting";

  let { html, onready }: { html: string; onready?: () => void } = $props();

  let content: HTMLDivElement;
  let highlightGeneration = 0;

  function declaredLanguage(code: HTMLElement): string | undefined {
    return [...code.classList]
      .find((className) => className.startsWith("language-"))
      ?.slice("language-".length);
  }

  $effect(() => {
    html;
    const generation = ++highlightGeneration;

    void tick().then(async () => {
      const highlighted = await Promise.all(
        [...content.querySelectorAll<HTMLElement>("pre > code")].map(async (code) => {
          const language = declaredLanguage(code);
          if (!language) return null;
          try {
            const result = await highlightedCode(code.textContent ?? "", language);
            return { code, html: result.html };
          } catch {
            return null;
          }
        })
      );
      if (generation !== highlightGeneration) return;

      for (const result of highlighted) {
        if (!result || !content.contains(result.code)) continue;
        result.code.innerHTML = result.html;
        result.code.classList.add("hljs");
      }
      onready?.();
    });

    return () => {
      if (highlightGeneration === generation) highlightGeneration += 1;
    };
  });
</script>

<div class="rich-text-viewer">
  <div bind:this={content} class="rich-text-content">{@html html}</div>
</div>
