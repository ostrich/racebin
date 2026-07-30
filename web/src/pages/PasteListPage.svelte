<script lang="ts">
  import { onMount } from "svelte";
  import { requestApi } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import Pagination from "../components/Pagination.svelte";
  import PasteFilters from "../components/PasteFilters.svelte";
  import PasteRows from "../components/PasteRows.svelte";
  import { showNotice } from "../notices";
  import type { Page, Paste } from "../types";

  let { mine, query }: { mine: boolean; query: URLSearchParams } = $props();
  let page = $state<Page<Paste> | null>(null);
  let error = $state("");

  onMount(() => {
    const params = new URLSearchParams(query);
    params.set("page_size", "50");
    if (mine) params.set("mine", "true");
    else params.set("visibility", "public");
    void requestApi<Page<Paste>>(`/pastes?${params}`)
      .then(result => { page = result; })
      .catch(reason => {
        error = reason instanceof Error ? reason.message : "Unable to load pastes";
        showNotice(error, "error");
      });
  });
</script>

<section>
  <div class="page-heading">
    <div><p class="eyebrow">{mine ? "Workspace" : "Public"}</p><h1>{mine ? "My pastes" : "Explore"}</h1></div>
    {#if mine}<Link class="button primary" href="/pastes/new"><Icon name="plus"/> New paste</Link>{/if}
  </div>
  <PasteFilters params={query} mode={mine ? "mine" : "explore"}/>
  {#if page}
    <p class="result-count">{page.total_items} paste{page.total_items === 1 ? "" : "s"}</p>
    <PasteRows items={page.items} manage={mine} filterable/>
    <Pagination {page}/>
  {:else if error}
    <div class="empty compact"><p>{error}</p></div>
  {:else}
    <p class="muted">Loading pastes…</p>
  {/if}
</section>
