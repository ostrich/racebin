<script lang="ts">
  import { onMount } from "svelte";
  import { getPaste, getPasteSource, pasteQrUrl, readPaste } from "../api";
  import AttachmentList from "../components/AttachmentList.svelte";
  import CodeViewer from "../components/CodeViewer.svelte";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import RichTextViewer from "../components/RichTextViewer.svelte";
  import { formatDate, pasteDisplayTitle, pasteFormatLabel } from "../format";
  import { showNotice } from "../notices";
  import { holdNavigation } from "../navigation";
  import { appState } from "../state";
  import type { Paste } from "../types";

  let { pasteId }: { pasteId: string } = $props();
  let paste = $state<Paste | null>(null);
  let error = $state("");
  let wrapLines = $state(false);
  let markdownView = $state<"rendered" | "markdown">("rendered");
  let horizontalOverflow = $state(false);
  let codeViewer = $state<{ preparePrint(): Promise<void> }>();
  let preparingPrint = $state(false);
  const initialLoadReady = holdNavigation();
  let own = $derived(Boolean(
    paste?.source_url
  ));
  let showWrapOption = $derived(Boolean(
    (paste?.content_kind === "text" || markdownView === "markdown") && (horizontalOverflow || wrapLines)
  ));
  let printLanguage = $derived(paste
    ? $appState.languages.find(language => language.id === paste?.language)?.label ?? paste.language
    : "");

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

  async function printPaste(): Promise<void> {
    if (!paste || preparingPrint) return;
    preparingPrint = true;
    try {
      if (paste.content_kind === "text") await codeViewer?.preparePrint();
      window.print();
    } finally {
      preparingPrint = false;
    }
  }
</script>

{#if paste}
  <article class="paste-view">
      <div class="page-heading" class:has-view-options={paste.content_kind === "markdown" || showWrapOption}>
        <div>
          <p class="eyebrow">{paste.visibility} · {pasteFormatLabel(paste)}</p>
          <h1>{pasteDisplayTitle(paste)}</h1>
          <p class="paste-print-metadata">
            <span><strong>Visibility:</strong> {paste.visibility.charAt(0).toUpperCase() + paste.visibility.slice(1)}</span>
            <span aria-hidden="true">·</span>
            {#if paste.content_kind === "text"}
              <span><strong>Language:</strong> {printLanguage}</span>
            {:else}
              <span><strong>Format:</strong> {pasteFormatLabel(paste)}</span>
            {/if}
          </p>
        </div>
        <div class="actions">
          {#if paste.raw_url}<a class="button" href={paste.raw_url} target="_blank" rel="noopener noreferrer">Raw</a>{/if}
          <button class="button" type="button" onclick={copyContent}><Icon name="copy"/> Copy</button>
          <button class="button" type="button" disabled={preparingPrint} onclick={printPaste}><Icon name="printer"/> {preparingPrint ? "Preparing…" : "Print"}</button>
          {#if paste.archive_url}<a class="button" href={paste.archive_url}>ZIP</a>{/if}
          {#if $appState.config.qr_codes_enabled}<a class="button" href={pasteQrUrl($appState.config.api_base_url ?? "/api/v1", paste.id)}>QR</a>{/if}
          {#if own}<Link class="button primary" href={`/pastes/${paste.id}/edit`}><Icon name="edit-3"/> Edit</Link>{/if}
        </div>
      </div>
      {#if paste.content_kind === "markdown"}
        <div class="markdown-view-controls">
          <div class="paste-view-options markdown-wrap-slot">
            {#if showWrapOption}
              <label class="paste-wrap-toggle">
                <input type="checkbox" bind:checked={wrapLines}/>
                <span>Wrap</span>
              </label>
            {/if}
          </div>
          <div class="paste-view-options markdown-view-options" role="group" aria-label="Paste representation">
            <button type="button" class:active={markdownView === "rendered"} onclick={() => { markdownView = "rendered"; }}>Rendered</button>
            <button type="button" class:active={markdownView === "markdown"} onclick={() => { markdownView = "markdown"; }}>Markdown</button>
          </div>
        </div>
      {:else if showWrapOption}
        <div class="paste-view-options">
          <label class="paste-wrap-toggle">
            <input type="checkbox" bind:checked={wrapLines}/>
            <span>Wrap</span>
          </label>
        </div>
      {/if}
      {#if paste.content_kind === "markdown" && markdownView === "rendered"}
        <RichTextViewer html={paste.rendered_html ?? ""} onready={initialLoadReady}/>
      {:else}
        <CodeViewer bind:this={codeViewer} code={paste.content} language={paste.content_kind === "markdown" ? "markdown" : paste.language} wrap={wrapLines}
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
        <span>{paste.read_count} view{paste.read_count === 1 ? "" : "s"}</span>
      </footer>
  </article>
{:else if error}
  <section class="empty"><h1>Unable to load this paste</h1><p>{error}</p><Link class="button" href="/">Return home</Link></section>
{:else}
  <p class="muted">Loading paste…</p>
{/if}
