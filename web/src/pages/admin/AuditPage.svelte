<script lang="ts">
  import { listAuditEvents, type AuditEvent } from "../../api";
  import AdminNav from "../../components/AdminNav.svelte";
  import Icon from "../../components/Icon.svelte";
  import Pagination from "../../components/Pagination.svelte";
  import { formatDate } from "../../format";
  import { holdNavigation, navigate } from "../../navigation";
  import type { Page } from "../../types";

  let { query }: { query: URLSearchParams } = $props();
  let page = $state<Page<AuditEvent> | null>(null);
  let error = $state("");
  let search = $state("");
  let initialLoadReady: (() => void) | null = holdNavigation();
  let generation = 0;

  $effect(() => {
    const source = new URLSearchParams(query);
    source.set("page_size", "25");
    const current = ++generation;
    const ready = initialLoadReady;
    initialLoadReady = null;
    search = query.get("search") ?? "";
    void listAuditEvents(source)
      .then(value => { if (current === generation) { page = value; error = ""; } })
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load audit log"; })
      .finally(() => ready?.());
  });

  async function applySearch(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const params = new URLSearchParams(query);
    if (search.trim()) params.set("search", search.trim()); else params.delete("search");
    params.delete("page");
    await navigate(`/admin/audit?${params}`);
  }
</script>

<section class="page-layout">
  <div class="page-heading">
    <div><p class="eyebrow">Owner</p><h1>Audit log</h1></div>
  </div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content">
      <form class="list-filter-bar" onsubmit={applySearch}><label class="field list-filter-search"><span>Search</span><input type="search" placeholder="Actor, action, or target" bind:value={search}></label><button class="button primary" type="submit"><Icon name="search"/> Search</button></form>
      {#if error}
        <section class="empty"><p>{error}</p></section>
      {:else if page}
        <p class="result-count">{page.total_items} events</p>
        <div class="panel data-list">
          {#each page.items as event (event.id)}
            <article class="data-row">
              <div>
                <strong>{event.action.replaceAll(".", " ")}</strong>
                <small>{event.actor_username} · {formatDate(event.created_at)}{event.target_label ? ` · ${event.target_label}` : event.target_id ? ` · ${event.target_type} ${event.target_id}` : ""}</small>
              </div>
            </article>
          {:else}
            <div class="empty"><p>No audit events match.</p></div>
          {/each}
        </div>
        <Pagination {page} params={query}/>
      {:else}<p class="muted">Loading audit events…</p>{/if}
    </div>
  </div>
</section>
