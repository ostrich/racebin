<script lang="ts">
  import { onMount, tick } from "svelte";
  import { highlightedCode } from "../highlighting";

  let {
    code,
    language,
    wrap = false,
    onready
  }: {
    code: string;
    language: string;
    wrap?: boolean;
    onready?: () => void;
  } = $props();
  let viewport: HTMLDivElement;
  let gutter: HTMLDivElement;
  let highlighted: HTMLElement;
  let floatingScrollbar: HTMLDivElement;
  let floatingContent: HTMLDivElement;
  let html = $state("");
  let lineOffsets = $state<number[]>([]);
  let floatingVisible = $state(false);
  let floatingLeft = $state(0);
  let floatingWidth = $state(0);
  let revision = 0;
  let count = $derived(code.split("\n").length);
  let lines = $derived(Array.from({ length: count }, (_, index) => index + 1).join("\n"));
  let width = $derived(`${Math.max(4, String(count).length + 2)}ch`);

  function updateLineOffsets(): void {
    if (!wrap || !gutter || !highlighted) return;
    const gutterTop = gutter.getBoundingClientRect().top;
    lineOffsets = [...highlighted.querySelectorAll<HTMLElement>(".line-anchor")]
      .map(anchor => anchor.getBoundingClientRect().top - gutterTop);
  }

  function updateFloatingScrollbar(): void {
    if (!viewport || !floatingScrollbar || !floatingContent) return;
    const bounds = viewport.getBoundingClientRect();
    const overflowing = !wrap && viewport.scrollWidth > viewport.clientWidth + 1;
    floatingVisible = overflowing
      && bounds.top < window.innerHeight
      && bounds.bottom > window.innerHeight;
    floatingLeft = Math.max(0, bounds.left);
    floatingWidth = Math.max(0, Math.min(window.innerWidth, bounds.right) - floatingLeft);
    floatingContent.style.width = `${viewport.scrollWidth}px`;
    floatingScrollbar.scrollLeft = viewport.scrollLeft;
  }

  function updateLayout(): void {
    updateFloatingScrollbar();
    updateLineOffsets();
  }

  function syncFromViewport(): void {
    floatingScrollbar.scrollLeft = viewport.scrollLeft;
  }

  function syncFromFloatingScrollbar(): void {
    viewport.scrollLeft = floatingScrollbar.scrollLeft;
  }

  onMount(() => {
    const observer = new ResizeObserver(updateLayout);
    observer.observe(viewport);
    window.addEventListener("scroll", updateFloatingScrollbar, { passive: true });
    window.addEventListener("resize", updateLayout, { passive: true });
    updateLayout();
    return () => {
      observer.disconnect();
      window.removeEventListener("scroll", updateFloatingScrollbar);
      window.removeEventListener("resize", updateLayout);
    };
  });

  $effect(() => {
    wrap;
    requestAnimationFrame(updateLayout);
  });

  $effect(() => {
    const current = ++revision;
    void highlightedCode(code, language).then(result => {
      if (current !== revision) return;
      html = result.html;
      void tick().then(() => {
        updateLayout();
        onready?.();
      });
    });
  });
</script>

<div class:wrap-lines={wrap} class="paste-code-shell">
  <div class="paste-code" style={`--line-number-width:${width}`}>
    <div bind:this={gutter} class="line-numbers" class:wrapped={wrap} aria-hidden="true">
      {#if wrap}
        {#each lineOffsets as offset, index}
          <span style={`top:${offset}px`}>{index + 1}</span>
        {/each}
      {:else}{lines}{/if}
    </div>
    <div bind:this={viewport} class="paste-code-content-scroll" onscroll={syncFromViewport}>
      <pre class="content"><code bind:this={highlighted} class="hljs">{@html wrap
        ? `<span class="line-anchor"></span>${html.replaceAll("\n", "\n<span class=\"line-anchor\"></span>")}`
        : html}</code></pre>
    </div>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (Native scroll region needs keyboard focus.) -->
  <div bind:this={floatingScrollbar} class="paste-floating-scrollbar"
    class:visible={floatingVisible} style={`left:${floatingLeft}px;width:${floatingWidth}px`}
    role="region" aria-label="Horizontal paste scrollbar" tabindex="0"
    onscroll={syncFromFloatingScrollbar}>
    <div bind:this={floatingContent}></div>
  </div>
</div>
