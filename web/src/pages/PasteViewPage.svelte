<script lang="ts">
  import { onMount } from "svelte";
  import { deletePaste, getPaste, getPasteSource, pasteQrUrl, readPaste } from "../api";
  import AttachmentList from "../components/AttachmentList.svelte";
  import CodeViewer from "../components/CodeViewer.svelte";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import RichTextViewer from "../components/RichTextViewer.svelte";
  import { formatDate, pasteDisplayTitle, pasteFormatLabel } from "../format";
  import { confirmAction } from "../confirmations";
  import { showNotice } from "../notices";
  import { holdNavigation, navigate } from "../navigation";
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
  let deleting = $state(false);
  const initialLoadReady = holdNavigation();
  let canManage = $derived(Boolean(
    paste?.source_url
  ));
  let showWrapOption = $derived(Boolean(
    (paste?.format === "text" || markdownView === "markdown") && (horizontalOverflow || wrapLines)
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
      if (paste.format === "text") await codeViewer?.preparePrint();
      window.print();
    } finally {
      preparingPrint = false;
    }
  }

  async function removePaste(): Promise<void> {
    if (!paste || deleting || !canManage || !(await confirmAction({ title: "Delete paste?", message: "This paste and its attachments will be permanently deleted.", confirmLabel: "Delete paste", dangerous: true }))) return;
    deleting = true;
    try {
      await deletePaste(paste.id, paste._etag ?? "*");
      showNotice("Paste deleted.");
      await navigate("/pastes");
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to delete paste", "error");
      deleting = false;
    }
  }
</script>

{#if paste}
  <article class="paste-view page-layout">
      <div class="page-heading">
        <div>
          <p class="eyebrow">{paste.visibility} · {pasteFormatLabel(paste)}</p>
          <h1>{pasteDisplayTitle(paste)}</h1>
          <p class="paste-print-metadata">
            <span><strong>Visibility:</strong> {paste.visibility.charAt(0).toUpperCase() + paste.visibility.slice(1)}</span>
            <span aria-hidden="true">·</span>
            {#if paste.format === "text"}
              <span><strong>Language:</strong> {printLanguage}</span>
            {:else}
              <span><strong>Format:</strong> {pasteFormatLabel(paste)}</span>
            {/if}
          </p>
        </div>
        <div class="actions page-heading-actions">
          {#if paste.format === "markdown"}
            <div class="paste-view-options markdown-wrap-slot">
              {#if showWrapOption}
                <label class="paste-wrap-toggle">
                  <input type="checkbox" bind:checked={wrapLines}/>
                  <span>Wrap</span>
                </label>
              {/if}
            </div>
            <div class="paste-view-options markdown-view-options segmented-control" role="group" aria-label="Paste representation">
              <button type="button" class:active={markdownView === "rendered"} onclick={() => { markdownView = "rendered"; }}>Rendered</button>
              <button type="button" class:active={markdownView === "markdown"} onclick={() => { markdownView = "markdown"; }}>Markdown</button>
            </div>
          {:else if showWrapOption}
            <div class="paste-view-options">
              <label class="paste-wrap-toggle">
                <input type="checkbox" bind:checked={wrapLines}/>
                <span>Wrap</span>
              </label>
            </div>
          {/if}
          {#if paste.raw_url}<a class="icon-button" href={paste.raw_url} target="_blank" rel="noopener noreferrer" title="Raw" aria-label="Raw"><Icon name="file-code"/></a>{/if}
          <button class="icon-button" type="button" title="Copy" aria-label="Copy" onclick={copyContent}><Icon name="copy"/></button>
          <button class="icon-button" type="button" title={preparingPrint ? "Preparing print" : "Print"} aria-label={preparingPrint ? "Preparing print" : "Print"} disabled={preparingPrint} onclick={printPaste}><Icon name="printer"/></button>
          {#if paste.archive_url}<a class="icon-button" href={paste.archive_url} title="Download ZIP" aria-label="Download ZIP"><Icon name="archive"/></a>{/if}
          {#if $appState.config.qr_codes_enabled}<a class="icon-button" href={pasteQrUrl($appState.config.api_base_url ?? "/api/v1", paste.id)} title="QR code" aria-label="QR code"><Icon name="qr-code"/></a>{/if}
          {#if canManage}<Link class="icon-button primary" href={`/pastes/${paste.id}/edit`} title="Edit" aria-label="Edit"><Icon name="edit-3"/></Link>{/if}
          {#if canManage}<button class="icon-button danger" type="button" title={deleting ? "Deleting paste" : "Delete"} aria-label={deleting ? "Deleting paste" : "Delete"} disabled={deleting} onclick={removePaste}><Icon name="trash-2"/></button>{/if}
        </div>
      </div>
      {#if paste.format === "markdown" && markdownView === "rendered"}
        <RichTextViewer html={paste.rendered_html ?? ""} onready={initialLoadReady}/>
      {:else}
        <CodeViewer bind:this={codeViewer} code={paste.content} language={paste.format === "markdown" ? "markdown" : paste.language} wrap={wrapLines}
          onready={initialLoadReady}
          onoverflowchange={(overflowing) => { horizontalOverflow = overflowing; }}/>
      {/if}
      {#if paste.attachments.length}
        <section><h2>Attachments</h2>
          <AttachmentList pasteId={paste.id} attachments={paste.attachments} canDelete={canManage} etag={paste._etag}
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
