<script lang="ts">
  import { highlightedCode } from "../highlighting";

  let { code, language }: { code: string; language: string } = $props();
  let html = $state("");
  let revision = 0;
  let count = $derived(code.split("\n").length);
  let lines = $derived(Array.from({ length: count }, (_, index) => index + 1).join("\n"));
  let width = $derived(`${Math.max(4, String(count).length + 2)}ch`);

  $effect(() => {
    const current = ++revision;
    void highlightedCode(code, language).then(result => {
      if (current === revision) html = result.html;
    });
  });
</script>

<div class="paste-code" style={`--line-number-width:${width}`}>
  <div class="line-numbers" aria-hidden="true">{lines}</div>
  <pre class="content"><code class="hljs">{@html html}</code></pre>
</div>
