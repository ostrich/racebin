<script lang="ts">
  import { createApiKey, deleteApiKey, listApiKeys, updateApiKey } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import Pagination from "../components/Pagination.svelte";
  import { formatDate } from "../format";
  import { confirmAction } from "../confirmations";
  import { showNotice } from "../notices";
  import { holdNavigation, navigate } from "../navigation";
  import { appState } from "../state";
  import type { ApiKey, Page } from "../types";

  let { query }: { query: URLSearchParams } = $props();

  let scopes = $derived($appState.config.scopes.filter(scope =>
    $appState.session.user?.role === "admin" ||
    $appState.session.user?.role === "owner" ||
    !scope.id.endsWith(":manage")
  ));
  let page = $state<Page<ApiKey> | null>(null);
  let keys = $derived(page?.items ?? []);
  let loading = $state(true);
  let submitting = $state(false);
  let search = $state("");
  let initialLoadReady: (() => void) | null = holdNavigation();
  let generation = 0;

  async function load(source = query): Promise<void> {
    const current = ++generation;
    loading = true;
    try {
      const params = new URLSearchParams(source);
      params.set("page_size", "25");
      const value = await listApiKeys(params);
      if (current === generation) page = value;
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Unable to load API keys", "error");
    } finally {
      if (current === generation) loading = false;
      initialLoadReady?.();
      initialLoadReady = null;
    }
  }

  async function toggle(key: ApiKey, enabled: boolean): Promise<void> {
    const previous = key.enabled;
    key.enabled = enabled;
    try {
      await updateApiKey(key.id, enabled);
    } catch (error) {
      key.enabled = previous;
      if (page) page = { ...page, items: [...page.items] };
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }

  async function remove(key: ApiKey): Promise<void> {
    if (!(await confirmAction({ title: "Delete API key?", message: `The key “${key.name}” will stop working immediately.`, confirmLabel: "Delete key", dangerous: true }))) return;
    try {
      await deleteApiKey(key.id);
      if (page) page = { ...page, items: page.items.filter(candidate => candidate.id !== key.id), total_items: page.total_items - 1 };
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }

  async function create(event: SubmitEvent): Promise<void> {
    const form = event.currentTarget as HTMLFormElement;
    const data = new FormData(form);
    submitting = true;
    try {
      const result = await createApiKey({
        name: String(data.get("name") ?? ""),
        scopes: data.getAll("scopes").map(String)
      });
      prompt("API key created. Store it now; it will not be shown again.", result.token);
      form.reset();
      await load();
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Unable to create API key", "error");
    } finally {
      submitting = false;
    }
  }

  async function applySearch(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const params = new URLSearchParams(query);
    if (search.trim()) params.set("search", search.trim()); else params.delete("search");
    params.delete("page");
    await navigate(`/account?${params}`);
  }

  function setFilter(key: string, value: string): void {
    const params = new URLSearchParams(query);
    if (value) params.set(key, value); else params.delete(key);
    params.delete("page");
    void navigate(`/account?${params}`);
  }

  $effect(() => {
    search = query.get("search") ?? "";
    void load(new URLSearchParams(query));
  });
</script>

<section class="page-layout">
  <div class="page-heading">
    <div><p class="eyebrow">Settings</p><h1>Account</h1></div>
    <div class="page-heading-actions"><Link class="button" href="/account/password">Change password</Link></div>
  </div>
  <section class="panel">
    <h2>API keys</h2><p class="muted">Tokens are shown once when created.</p>
    <form class="list-filter-bar account-key-filters" onsubmit={applySearch}>
      <label class="field list-filter-search"><span>Search</span><input type="search" placeholder="Name, prefix, or privilege" bind:value={search}></label>
      <button class="button primary" type="submit"><Icon name="search"/> Search</button>
      <label class="field list-filter-select"><span>Status</span><select value={query.get("status") ?? ""} onchange={event => setFilter("status", event.currentTarget.value)}><option value="">Any status</option><option value="enabled">Enabled</option><option value="disabled">Disabled</option></select></label>
      <label class="field list-filter-select"><span>Sort</span><select value={query.get("sort") ?? "created"} onchange={event => setFilter("sort", event.currentTarget.value)}><option value="created">Created</option><option value="name">Name</option><option value="used">Last used</option></select></label>
      <label class="field list-filter-select"><span>Direction</span><select value={query.get("direction") ?? "desc"} onchange={event => setFilter("direction", event.currentTarget.value)}><option value="desc">Descending</option><option value="asc">Ascending</option></select></label>
    </form>
    {#if page}<p class="result-count">{page.total_items} API keys</p>{/if}
    <div class="key-list">
      {#if loading}<p class="muted">Loading API keys…</p>
      {:else if !keys.length}<p class="empty compact">No API keys.</p>
      {:else}
        {#each keys as key (key.id)}
          <div class="key-row">
            <div>
              <strong>{key.name}</strong><code>rbk_{key.token_prefix}_...</code>
              <small>{key.scopes.join(", ") || "No scopes"} · Created {formatDate(key.created_at)}</small>
            </div>
            <label class="switch">
              <input type="checkbox" checked={key.enabled}
                aria-label={`Enable ${key.name}`}
                onchange={(event) => toggle(key, event.currentTarget.checked)}/>
              <span></span>
            </label>
            <button class="icon-button" title="Delete API key" aria-label={`Delete ${key.name}`}
              type="button" onclick={() => remove(key)}><Icon name="trash-2"/></button>
          </div>
        {/each}
      {/if}
    </div>
    {#if page}<Pagination {page} params={query}/>{/if}
    <form class="key-form" onsubmit={(event) => { event.preventDefault(); void create(event); }}>
      <label class="field"><span>Name</span><input name="name" required maxlength="100"/></label>
      <fieldset><legend>Scopes</legend><div class="scope-options">
        {#each scopes as scope}
          <label class="check" title={scope.description}><input type="checkbox" name="scopes" value={scope.id}/><span>{scope.id}</span></label>
        {/each}
      </div></fieldset>
      <button class="button primary" type="submit" disabled={submitting}><Icon name="key-round"/> {submitting ? "Creating…" : "Create key"}</button>
    </form>
  </section>
</section>
