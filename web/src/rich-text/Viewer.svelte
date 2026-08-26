<script lang="ts">
  import { onMount } from "svelte";
  let { html, onready }: { html: string; onready?: () => void } = $props();
  onMount(() => { onready?.(); });
</script>

<div class="rich-text-viewer"><div class="rich-text-content">{@html html}</div></div>
