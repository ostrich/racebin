<script lang="ts">
  import { deleteAdminApiKey, listAdminApiKeys, updateAdminApiKey } from "../api";
  import AdminNav from "../components/AdminNav.svelte";
  import Icon from "../components/Icon.svelte";
  import Pagination from "../components/Pagination.svelte";
  import { confirmAction } from "../app/confirmations";
  import { holdNavigation, navigate } from "../navigation";
  import { showNotice } from "../app/notices";
  import type { ApiKey, Page } from "../types";

  let { query }: { query: URLSearchParams } = $props();
  let page = $state<Page<ApiKey> | null>(null);
  let error = $state("");
  let search = $state("");
  let initialLoadReady: (() => void) | null = holdNavigation();
  let generation = 0;

  async function load(source = query): Promise<void> {
    const current = ++generation;
    const params = new URLSearchParams(source);
    params.set("page_size", "25");
    const value = await listAdminApiKeys(params);
    if (current === generation) {
      page = value;
      error = "";
    }
  }

  $effect(() => {
    const source = new URLSearchParams(query);
    const ready = initialLoadReady;
    initialLoadReady = null;
    search = query.get("search") ?? "";
    void load(source)
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load API keys"; })
      .finally(() => ready?.());
  });

  function ownerName(key: ApiKey): string {
    return key.owner_username ?? "No owner";
  }

  async function applySearch(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const params = new URLSearchParams(query);
    if (search.trim()) params.set("search", search.trim()); else params.delete("search");
    params.delete("page");
    await navigate(`/admin/api-keys?${params}`);
  }

  function setFilter(key: string, value: string): void {
    const params = new URLSearchParams(query);
    if (value) params.set(key, value); else params.delete(key);
    params.delete("page");
    void navigate(`/admin/api-keys?${params}`);
  }

  async function toggle(key: ApiKey): Promise<void> {
    try {
      await updateAdminApiKey(key.id, !key.enabled);
      await load();
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to update API key", "error");
    }
  }

  async function remove(key: ApiKey): Promise<void> {
    if (!(await confirmAction({ title: "Delete API key?", message: `The key “${key.name}” will stop working immediately.`, confirmLabel: "Delete key", dangerous: true }))) return;
    try {
      await deleteAdminApiKey(key.id);
      await load();
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to delete API key", "error");
    }
  }
</script>

<section class="page-layout">
  <div class="page-heading">
    <div><p class="eyebrow">Administration</p><h1>API keys</h1></div>
  </div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content">
      <form class="list-filter-bar" onsubmit={applySearch}>
        <label class="field list-filter-search"><span>Search</span><input type="search" placeholder="Name, owner, prefix, or privilege" bind:value={search}></label>
        <button class="button primary" type="submit"><Icon name="search"/> Search</button>
        <label class="field list-filter-select"><span>Status</span><select value={query.get("status") ?? ""} onchange={event => setFilter("status", event.currentTarget.value)}><option value="">Any status</option><option value="enabled">Enabled</option><option value="disabled">Disabled</option></select></label>
        <label class="field list-filter-select"><span>Sort</span><select value={query.get("sort") ?? "created"} onchange={event => setFilter("sort", event.currentTarget.value)}><option value="created">Created</option><option value="name">Name</option><option value="owner">Owner</option><option value="used">Last used</option></select></label>
        <label class="field list-filter-select"><span>Direction</span><select value={query.get("direction") ?? "desc"} onchange={event => setFilter("direction", event.currentTarget.value)}><option value="desc">Descending</option><option value="asc">Ascending</option></select></label>
      </form>
      {#if error}
        <section class="empty"><p>{error}</p></section>
      {:else if page}
        <p class="result-count">{page.total_items} API keys</p>
        <div class="panel data-list">
          {#each page.items as key (key.id)}
            <article class="data-row">
              <div>
                <strong>{key.name}</strong>
                <small>{ownerName(key)} · <code>{key.token_prefix}</code></small>
                <div class="badge-group">{#each key.scopes as scope}<span class="badge">{scope}</span>{/each}</div>
              </div>
              <div class="row-actions">
                <button class="button" onclick={() => toggle(key)}>{key.enabled ? "Disable" : "Enable"}</button>
                <button class="icon-button" title="Delete API key" aria-label="Delete API key" onclick={() => remove(key)}><Icon name="trash-2"/></button>
              </div>
            </article>
          {:else}
            <div class="empty"><p>No API keys match.</p></div>
          {/each}
        </div>
        <Pagination {page} params={query}/>
      {:else}<p class="muted">Loading API keys…</p>{/if}
    </div>
  </div>
</section>
