<script lang="ts">
  import { onMount } from "svelte";
  import { listPastes } from "../api";
  import Link from "../components/Link.svelte";
  import PasteRows from "../components/PasteRows.svelte";
  import { showNotice } from "../notices";
  import { holdNavigation } from "../navigation";
  import { appState } from "../state";
  import type { Page, Paste } from "../types";

  let page = $state<Page<Paste> | null>(null);
  const initialLoadReady = holdNavigation();

  onMount(() => {
    void listPastes(new URLSearchParams({ visibility: "public", page_size: "8" }))
      .then(result => { page = result; })
      .catch(error => showNotice(error instanceof Error ? error.message : "Unable to load pastes", "error"))
      .finally(initialLoadReady);
  });
</script>

<section class="welcome">
  <div>
    <p class="eyebrow">Simple sharing for code, notes, and files.</p>
    <h1>{$appState.config.site_name}</h1>
    <p>Browse public pastes below, or sign in to create syntax-highlighted and rich-text pastes of your own.</p>
    <div class="actions">
      <Link class="button primary" href="/explore">Explore pastes</Link>
      <Link class="button" href="/login">Log in</Link>
    </div>
  </div>
</section>
<section>
  <div class="section-heading"><h2>Recently shared</h2><Link href="/explore">View all</Link></div>
  {#if page}<PasteRows items={page.items}/>{:else}<p class="muted">Loading pastes…</p>{/if}
</section>
