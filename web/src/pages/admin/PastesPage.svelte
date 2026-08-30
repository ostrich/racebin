<script lang="ts">
  import { listAdminPastes } from "../../api";
  import AdminNav from "../../components/AdminNav.svelte";
  import Pagination from "../../components/Pagination.svelte";
  import PasteFilters from "../../components/PasteFilters.svelte";
  import PasteRows from "../../components/PasteRows.svelte";
  import { showNotice } from "../../app/notices";
  import { cachedQuery, loadQuery } from "../../app/queryCache";
  import { holdNavigation } from "../../navigation";
  import { appState } from "../../app/state";
  import type { Page, Paste } from "../../types";

  let { query }: { query: URLSearchParams } = $props();
  function pastePath(requestedQuery: URLSearchParams): string {
    const params = new URLSearchParams(requestedQuery);
    if (params.has("search")) {
      params.set("q", params.get("search") ?? "");
      params.delete("search");
    }
    for (const key of ["created_after", "created_before"]) {
      const value = params.get(key);
      if (value && Number.isFinite(Number(value))) {
        params.set(key, new Date(Number(value) * 1000).toISOString());
      }
    }
    params.set("page_size", String($appState.config.max_page_size));
    return `/admin/pastes?${params}`;
  }

  function initialState(): {
    page: Page<Paste> | null;
    query: URLSearchParams;
  } {
    const requestedQuery = new URLSearchParams(query);
    const cachedPage = cachedQuery<Page<Paste>>(pastePath(requestedQuery));
    const complete = Boolean(cachedPage);
    return {
      page: complete ? (cachedPage ?? null) : null,
      query: complete ? requestedQuery : new URLSearchParams()
    };
  }

  const initial = initialState();
  let page = $state<Page<Paste> | null>(initial.page);
  let appliedQuery = $state(initial.query);
  let loading = $state(false);
  let error = $state("");
  let ownerNames = $derived(
    new Map(
      page?.items
        .filter((paste) => paste.owner_id && paste.owner_username)
        .map((paste) => [paste.owner_id!, paste.owner_username!]) ?? []
    )
  );
  let loadGeneration = 0;
  let initialRouteReady: (() => void) | null = holdNavigation();

  $effect(() => {
    const requestedQuery = new URLSearchParams(query);
    const generation = ++loadGeneration;
    const routeReady = initialRouteReady ?? holdNavigation();
    initialRouteReady = null;
    loading = true;
    const requestedPastePath = pastePath(requestedQuery);
    const cachedPage = cachedQuery<Page<Paste>>(requestedPastePath);
    if (cachedPage) {
      page = cachedPage;
      appliedQuery = requestedQuery;
      error = "";
    }
    void loadQuery(requestedPastePath, () =>
      listAdminPastes(new URLSearchParams(requestedPastePath.split("?")[1]))
    )
      .then((result) => {
        if (generation !== loadGeneration) return;
        page = result;
        appliedQuery = requestedQuery;
        error = "";
      })
      .catch((reason) => {
        if (generation !== loadGeneration) return;
        const message = reason instanceof Error ? reason.message : "Unable to load pastes";
        if (!page) error = message;
        showNotice(message, "error");
      })
      .finally(() => {
        if (generation === loadGeneration) loading = false;
        routeReady();
      });
    return () => {
      if (generation === loadGeneration) loadGeneration += 1;
      routeReady();
    };
  });

  function pasteRemoved(paste: Paste): void {
    if (page)
      page = {
        ...page,
        items: page.items.filter((candidate) => candidate.id !== paste.id),
        total_items: page.total_items - 1
      };
  }
</script>

<section class="page-layout" aria-busy={loading}>
  <div class="page-heading">
    <div>
      <p class="eyebrow">Administration</p>
      <h1>All pastes</h1>
    </div>
  </div>
  <div class="section-layout">
    <AdminNav />
    <div class="section-content admin-paste-content">
      <PasteFilters params={appliedQuery} mode="admin" {ownerNames} />
      {#if page}
        <PasteRows
          items={page.items}
          context="admin"
          manage
          filterable
          {ownerNames}
          totalItems={page.total_items}
          onremoved={pasteRemoved}
        />
        <Pagination {page} params={appliedQuery} />
      {:else if error}<div class="empty compact"><p>{error}</p></div>
      {:else}<p class="muted">Loading pastes…</p>{/if}
    </div>
  </div>
</section>
