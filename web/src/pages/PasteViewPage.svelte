<script lang="ts">
  import { onMount } from "svelte";
  import { requestApi } from "../api";
  import AttachmentList from "../components/AttachmentList.svelte";
  import CodeViewer from "../components/CodeViewer.svelte";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { formatDate, pasteDisplayTitle, pasteFormatLabel } from "../format";
  import { showNotice } from "../notices";
  import { deferRouteReady } from "../router";
  import { appState } from "../state";
  import type { Paste } from "../types";

  let { pasteId }: { pasteId: string } = $props();
  let paste = $state<Paste | null>(null);
  let error = $state("");
  let wrapLines = $state(false);
  let horizontalOverflow = $state(false);
  const initialLoadReady = deferRouteReady();
  let own = $derived(Boolean(
    paste && $appState.session.user &&
    ($appState.session.user.id === paste.owner_id || $appState.session.user.role === "admin")
  ));
  let showWrapOption = $derived(Boolean(
    paste?.content_kind === "text" && (horizontalOverflow || wrapLines)
  ));

  onMount(() => {
    void requestApi<Paste>(`/pastes/${encodeURIComponent(pasteId)}/consume`)
      .then(result => { paste = result; })
      .catch(reason => {
        error = reason instanceof Error ? reason.message : "Unable to load paste";
        initialLoadReady();
      });
  });

  async function copyContent(): Promise<void> {
    if (!paste) return;
    await navigator.clipboard.writeText(paste.content);
    showNotice("Paste copied.");
  }
</script>

{#if paste}
  <article class="paste-view">
      <div class="page-heading" class:has-view-options={showWrapOption}>
        <div><p class="eyebrow">{paste.visibility} · {pasteFormatLabel(paste)}</p><h1>{pasteDisplayTitle(paste)}</h1></div>
        <div class="actions">
          <a class="button" href={`/api/v1/pastes/${encodeURIComponent(paste.id)}/raw`}>Raw</a>
          <button class="button" type="button" onclick={copyContent}><Icon name="copy"/> Copy</button>
          {#if paste.attachments.length}<a class="button" href={`/api/v1/pastes/${encodeURIComponent(paste.id)}/archive`}>ZIP</a>{/if}
          {#if $appState.config.qr_codes_enabled}<a class="button" href={`/api/v1/pastes/${encodeURIComponent(paste.id)}/qr`}>QR</a>{/if}
          {#if own}<Link class="button primary" href={`/pastes/${paste.id}/edit`}><Icon name="edit-3"/> Edit</Link>{/if}
        </div>
      </div>
      {#if showWrapOption}
        <div class="paste-view-options">
          <label class="paste-wrap-toggle">
            <input type="checkbox" bind:checked={wrapLines}/>
            <span>Wrap</span>
          </label>
        </div>
      {/if}
      {#if paste.content_kind === "rich_text" && paste.document}
        {#await import("../components/RichTextViewer.svelte") then module}
          {@const RichTextViewer = module.default}
          <RichTextViewer document={paste.document} onready={initialLoadReady}/>
        {/await}
      {:else}
        <CodeViewer code={paste.content} language={paste.language} wrap={wrapLines}
          onready={initialLoadReady}
          onoverflowchange={(overflowing) => { horizontalOverflow = overflowing; }}/>
      {/if}
      {#if paste.attachments.length}
        <section><h2>Attachments</h2>
          <AttachmentList pasteId={paste.id} attachments={paste.attachments} canDelete={own}
            ondelete={(attachment) => { if (paste) paste.attachments = paste.attachments.filter(item => item.id !== attachment.id); }}/>
        </section>
      {/if}
      <footer class="paste-stats">
        <span>Created {formatDate(paste.created_at)}</span>
        <span>Expires {formatDate(paste.expires_at)}</span>
        <span>{paste.read_count} reads</span>
      </footer>
  </article>
{:else if error}
  <section class="empty"><h1>Unable to load this paste</h1><p>{error}</p><Link class="button" href="/">Return home</Link></section>
{:else}
  <p class="muted">Loading paste…</p>
{/if}
