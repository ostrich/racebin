<script lang="ts">
  import type { Page } from "../types";
  import Link from "./Link.svelte";

  let { page, params }: { page: Page<unknown>; params?: URLSearchParams } = $props();
  let pages = $derived(Math.max(1, Math.ceil(page.total_items / page.page_size)));

  function pageUrl(number: number): string {
    const next = new URLSearchParams(params ?? location.search);
    next.set("page", String(number));
    return `${location.pathname}?${next}`;
  }
</script>

{#if pages > 1}
  <nav class="pagination" aria-label="Pagination">
    {#if page.page > 1}<Link class="button" href={pageUrl(page.page - 1)}>Previous</Link>{/if}
    <span>Page {page.page} of {pages}</span>
    {#if page.page < pages}<Link class="button" href={pageUrl(page.page + 1)}>Next</Link>{/if}
  </nav>
{/if}
