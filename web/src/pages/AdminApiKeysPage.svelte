<script lang="ts">
  import { onMount } from "svelte";
  import { deleteAdminApiKey, listAdminApiKeys, listAdminUsers, updateAdminApiKey } from "../api";
  import AdminNav from "../components/AdminNav.svelte";
  import Icon from "../components/Icon.svelte";
  import { holdNavigation } from "../navigation";
  import { showNotice } from "../notices";
  import type { AdminUser, ApiKey } from "../types";

  let keys = $state<ApiKey[]>([]);
  let users = $state<AdminUser[]>([]);
  let error = $state("");
  let search = $state("");
  const initialLoadReady = holdNavigation();
  let filtered = $derived(keys.filter(key =>
    `${key.name} ${key.token_prefix} ${ownerName(key)} ${key.scopes.join(" ")}`
      .toLowerCase().includes(search.toLowerCase())
  ));

  async function load(): Promise<void> {
    [keys, users] = await Promise.all([listAdminApiKeys(), listAdminUsers()]);
  }

  onMount(() => {
    void load()
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load API keys"; })
      .finally(initialLoadReady);
  });

  function ownerName(key: ApiKey): string {
    return users.find(user => user.id === key.user_id)?.username ?? "No owner";
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
    if (!confirm(`Delete ${key.name}?`)) return;
    try {
      await deleteAdminApiKey(key.id);
      await load();
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to delete API key", "error");
    }
  }
</script>

<section class="stack page-stack">
  <div class="page-heading">
    <div><p class="eyebrow">Administration</p><h1>API keys</h1></div>
  </div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content">
      <label class="field search-control"><span>Search</span><input type="search" placeholder="Name, owner, prefix, or privilege" bind:value={search}></label>
      {#if error}
        <section class="empty"><p>{error}</p></section>
      {:else}
        <div class="panel data-list">
          {#each filtered as key (key.id)}
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
      {/if}
    </div>
  </div>
</section>
