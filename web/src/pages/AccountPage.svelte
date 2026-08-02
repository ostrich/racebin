<script lang="ts">
  import { onMount } from "svelte";
  import { createApiKey, deleteApiKey, listApiKeys, updateApiKey } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { formatDate } from "../format";
  import { showNotice } from "../notices";
  import { holdNavigation } from "../navigation";
  import { appState } from "../state";
  import type { ApiKey } from "../types";

  let scopes = $derived($appState.config.scopes.filter(scope =>
    $appState.session.user?.role === "admin" || !scope.id.endsWith(":manage")
  ));
  let keys = $state<ApiKey[]>([]);
  let loading = $state(true);
  let submitting = $state(false);
  const initialLoadReady = holdNavigation();

  async function load(): Promise<void> {
    loading = true;
    try {
      keys = await listApiKeys();
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Unable to load API keys", "error");
    } finally {
      loading = false;
      initialLoadReady();
    }
  }

  async function toggle(key: ApiKey, enabled: boolean): Promise<void> {
    const previous = key.enabled;
    key.enabled = enabled;
    try {
      await updateApiKey(key.id, enabled);
    } catch (error) {
      key.enabled = previous;
      keys = [...keys];
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }

  async function remove(key: ApiKey): Promise<void> {
    if (!confirm("Delete this API key permanently?")) return;
    try {
      await deleteApiKey(key.id);
      keys = keys.filter(candidate => candidate.id !== key.id);
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

  onMount(() => { void load(); });
</script>

<section class="stack page-stack">
  <div class="page-heading">
    <div><p class="eyebrow">Settings</p><h1>Account</h1></div>
    <Link class="button" href="/account/password">Change password</Link>
  </div>
  <section class="panel">
    <h2>API keys</h2><p class="muted">Tokens are shown once when created.</p>
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
    <form class="key-form" onsubmit={(event) => { event.preventDefault(); void create(event); }}>
      <label><span>Name</span><input name="name" required maxlength="100"/></label>
      <fieldset><legend>Scopes</legend><div class="scope-options">
        {#each scopes as scope}
          <label class="check" title={scope.description}><input type="checkbox" name="scopes" value={scope.id}/><span>{scope.id}</span></label>
        {/each}
      </div></fieldset>
      <button class="button primary" type="submit" disabled={submitting}><Icon name="key-round"/> {submitting ? "Creating…" : "Create key"}</button>
    </form>
  </section>
</section>
