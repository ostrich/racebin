<script lang="ts">
  import { onMount } from "svelte";
  import { listPastes } from "../api";
  import Link from "../components/Link.svelte";
  import PasteRows from "../components/PasteRows.svelte";
  import { showNotice } from "../app/notices";
  import { holdNavigation } from "../navigation";
  import { appState } from "../app/state";
  import type { Page, Paste } from "../types";

  let page = $state<Page<Paste> | null>(null);
  const initialLoadReady = holdNavigation();

  onMount(() => {
    if (!$appState.config.public_explore_enabled) {
      initialLoadReady();
      return;
    }
    void listPastes(new URLSearchParams({ visibility: "public", page_size: "8" }))
      .then((result) => {
        page = result;
      })
      .catch((error) =>
        showNotice(error instanceof Error ? error.message : "Unable to load pastes", "error")
      )
      .finally(initialLoadReady);
  });
</script>

<section class="welcome">
  <div>
    <p class="eyebrow">Simple sharing for code, notes, and files.</p>
    <h1>{$appState.config.site_name}</h1>
    {#if $appState.config.public_explore_enabled}
      <p>
        Browse public pastes below, or sign in to create syntax-highlighted and rich-text pastes of
        your own.
      </p>
    {:else}
      <p>Sign in to create and manage code, notes, and file attachments.</p>
    {/if}
    <div class="actions">
      {#if $appState.config.public_explore_enabled}
        <Link class="button primary" href="/explore">Explore pastes</Link>
        <Link class="button" href="/login">Log in</Link>
      {:else}
        <Link class="button primary" href="/login">Log in</Link>
      {/if}
    </div>
  </div>
</section>
{#if $appState.config.public_explore_enabled}
  <section>
    <div class="section-heading">
      <h2>Recently shared</h2>
      <Link href="/explore">View all</Link>
    </div>
    {#if page}<PasteRows items={page.items} context="public" />{:else}<p class="muted">
        Loading pastes…
      </p>{/if}
  </section>
{/if}
