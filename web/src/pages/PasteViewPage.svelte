<script lang="ts">
  import { onMount } from "svelte";
  import { getPaste, getPasteSource, pasteQrUrl, readPaste } from "../api";
  import AttachmentList from "../components/AttachmentList.svelte";
  import CodeViewer from "../components/CodeViewer.svelte";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { formatDate, pasteDisplayTitle, pasteFormatLabel } from "../format";
  import { showNotice } from "../notices";
  import { holdNavigation } from "../navigation";
  import { appState } from "../state";
  import type { Paste } from "../types";

  let { pasteId }: { pasteId: string } = $props();
  let paste = $state<Paste | null>(null);
  let error = $state("");
  let wrapLines = $state(false);
  let horizontalOverflow = $state(false);
  const initialLoadReady = holdNavigation();
  let own = $derived(Boolean(
    paste?.source_url
  ));
  let showWrapOption = $derived(Boolean(
    paste?.content_kind === "text" && (horizontalOverflow || wrapLines)
  ));

  onMount(() => {
    void getPaste(pasteId)
      .then(metadata => metadata.source_url
        ? getPasteSource(pasteId)
        : readPaste(pasteId, crypto.randomUUID()).then(result => result.paste))
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

  function openRaw(): void {
    if (!paste) return;
    const url = URL.createObjectURL(new Blob([paste.content], { type: "text/plain;charset=utf-8" }));
    window.open(url, "_blank", "noopener,noreferrer");
    window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
  }
</script>

{#if paste}
  <article class="paste-view">
      <div class="page-heading" class:has-view-options={showWrapOption}>
        <div><p class="eyebrow">{paste.visibility} · {pasteFormatLabel(paste)}</p><h1>{pasteDisplayTitle(paste)}</h1></div>
        <div class="actions">
          <button class="button" type="button" onclick={openRaw}>Raw</button>
          <button class="button" type="button" onclick={copyContent}><Icon name="copy"/> Copy</button>
          {#if paste.archive_url}<a class="button" href={paste.archive_url}>ZIP</a>{/if}
          {#if $appState.config.qr_codes_enabled}<a class="button" href={pasteQrUrl($appState.config.api_base_url ?? "/api/v1", paste.id)}>QR</a>{/if}
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
          <AttachmentList pasteId={paste.id} attachments={paste.attachments} canDelete={own} etag={paste._etag}
            ondelete={(attachment, etag) => { if (paste) paste = { ...paste, _etag: etag ?? paste._etag, attachments: paste.attachments.filter(item => item.id !== attachment.id) }; }}/>
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
