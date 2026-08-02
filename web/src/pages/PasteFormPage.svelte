<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    convertPaste, createPaste, createPasteWithAttachments, deletePaste as deletePasteRequest,
    getPasteSource, listFolders, updatePaste, uploadAttachments,
    type Conversion, type CreatePasteInput, type FlatCreateInput, type UpdatePasteInput
  } from "../api";
  import AttachmentList from "../components/AttachmentList.svelte";
  import CodeEditor from "../components/CodeEditor.svelte";
  import ConversionDialog from "../components/ConversionDialog.svelte";
  import LanguagePicker from "../components/LanguagePicker.svelte";
  import Link from "../components/Link.svelte";
  import { pasteDisplayTitle } from "../format";
  import { normalizeLanguage } from "../highlighting";
  import { showNotice } from "../notices";
  import { clearUnsavedChangesGuard, guardUnsavedChanges, holdNavigation, navigate } from "../navigation";
  import { appState } from "../state";
  import type { Folder, FolderOverview, Paste, RichTextDocument } from "../types";

  type ContentKind = "text" | "rich_text";
  type ExpirationMode = "never" | "1h" | "12h" | "1d" | "1w" | "30d" | "1y" | "custom";

  let { pasteId }: { pasteId?: string } = $props();
  let paste = $state<Paste | null>(null);
  let loading = $state(false);
  let error = $state("");
  let files = $state<HTMLInputElement>();
  let conversionDialog: ConversionDialog;
  let title = $state("");
  let content = $state("");
  let document = $state<RichTextDocument>({ type: "doc", content: [{ type: "paragraph" }] });
  let richHtml = $state("");
  let contentKind = $state<ContentKind>("text");
  let folderId = $state("");
  let folders = $state<Folder[]>([]);
  let language = $state("auto");
  let visibility = $state("unlisted");
  let expirationMode = $state<ExpirationMode>("never");
  let expiresAt = $state("");
  let readLimit = $state("");
  let attachmentSelection = $state("");
  let submitting = $state(false);
  let switching = $state(false);
  let editorHeight = $state(410);
  let baseline = $state("");
  let initialized = $state(false);
  const initialLoadReady = holdNavigation();
  const drafts = new Map<ContentKind, string>();
  let canOrganize = $derived(!paste || paste.owner_id === $appState.session.user?.id);

  function trackEditorResize(node: HTMLElement): { destroy: () => void } {
    const observer = new ResizeObserver(() => {
      const resizedHeight = Number.parseFloat(node.style.height);
      if (Number.isFinite(resizedHeight) && Math.abs(resizedHeight - editorHeight) > 1) {
        editorHeight = Math.max(240, Math.round(resizedHeight));
      }
    });
    observer.observe(node);
    return { destroy: () => observer.disconnect() };
  }

  function localDateTime(date: Date): string {
    const part = (value: number) => String(value).padStart(2, "0");
    return `${date.getFullYear()}-${part(date.getMonth() + 1)}-${part(date.getDate())}`
      + `T${part(date.getHours())}:${part(date.getMinutes())}`;
  }

  function changeExpirationMode(event: Event): void {
    const mode = (event.currentTarget as HTMLSelectElement).value as ExpirationMode;
    expirationMode = mode;
    if (mode === "never" || mode === "custom") {
      expiresAt = "";
      return;
    }

    const expiration = new Date();
    if (mode === "1h") expiration.setHours(expiration.getHours() + 1);
    else if (mode === "12h") expiration.setHours(expiration.getHours() + 12);
    else if (mode === "1d") expiration.setDate(expiration.getDate() + 1);
    else if (mode === "1w") expiration.setDate(expiration.getDate() + 7);
    else if (mode === "30d") expiration.setDate(expiration.getDate() + 30);
    else expiration.setDate(expiration.getDate() + 365);
    expiration.setSeconds(0, 0);
    expiresAt = localDateTime(expiration);
  }

  function customizeExpiration(): void {
    if (expirationMode !== "never" && expirationMode !== "custom") {
      expirationMode = "custom";
    }
  }

  function richTextIsEmpty(value: string): boolean {
    if (!value.trim()) return true;
    const text = new DOMParser().parseFromString(value, "text/html").body.textContent ?? "";
    return !text.replaceAll("\u00a0", " ").trim();
  }

  function snapshot(selectedAttachments = attachmentSelection): string {
    const effectiveKind = contentKind === "rich_text" && richTextIsEmpty(richHtml)
      ? "text"
      : contentKind;
    return JSON.stringify({
      title, content, richHtml: effectiveKind === "rich_text" ? richHtml : null,
      contentKind: effectiveKind, folderId, language: effectiveKind === "text" ? language : null,
      visibility, expirationMode, expiresAt, readLimit, attachmentSelection: selectedAttachments
    });
  }
  let dirty = $derived(initialized && snapshot() !== baseline);

  $effect(() => {
    guardUnsavedChanges(() => dirty);
    return () => clearUnsavedChangesGuard();
  });

  function initialize(source?: Paste): void {
    paste = source ?? null;
    title = source?.title ?? "";
    content = source?.content ?? "";
    document = source?.document ?? { type: "doc", content: [{ type: "paragraph" }] };
    richHtml = typeof source?.document === "string" ? source.document : "";
    contentKind = source?.content_kind ?? "text";
    folderId = source?.folder_id ? String(source.folder_id) : (
      source ? "" : new URLSearchParams(location.search).get("folder_id") ?? ""
    );
    language = normalizeLanguage(source?.language ?? "auto") ?? "auto";
    visibility = source?.visibility ?? "unlisted";
    expirationMode = source?.expires_at ? "custom" : "never";
    expiresAt = source?.expires_at
      ? localDateTime(new Date(source.expires_at * 1000))
      : "";
    readLimit = source?.read_limit ? String(source.read_limit) : "";
    drafts.set(contentKind, content);
    void tick().then(() => {
      baseline = snapshot();
      initialized = true;
      initialLoadReady();
    });
  }

  onMount(() => {
    void listFolders()
      .then(result => { folders = result.items; })
      .catch(reason => showNotice(reason instanceof Error ? reason.message : "Unable to load folders", "error"));
    loading = Boolean(pasteId);
    if (!pasteId) {
      initialize();
      loading = false;
      return;
    }
    void getPasteSource(pasteId)
      .then(initialize)
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load paste"; })
      .finally(() => {
        loading = false;
        initialLoadReady();
      });
  });

  async function convert(
    sourceKind: "text" | "rich_text",
    targetKind: "text" | "rich_text"
  ): Promise<Conversion> {
    return convertPaste({
      source: sourceKind === "rich_text"
        ? { format: "rich_text", content: richHtml }
        : { format: "text", content, language },
      target_format: targetKind
    });
  }

  async function changeKind(event: Event): Promise<void> {
    const selector = event.currentTarget as HTMLSelectElement;
    const source = contentKind;
    const target = selector.value as ContentKind;
    selector.value = source;
    if (source === target) return;
    switching = true;
    try {
      if (source === "rich_text") {
        const converted = await convert("rich_text", "text");
        const convertedText = converted.body.content;
        if (convertedText && !(await conversionDialog.ask(target, convertedText))) return;
        drafts.set(target, convertedText);
        content = convertedText;
      } else if (target === "rich_text") {
        drafts.set(source, content);
        const converted = await convert("text", "rich_text");
        const convertedHtml = converted.body.content;
        if (content && !(await conversionDialog.ask(target, content))) return;
        document = convertedHtml;
        richHtml = convertedHtml;
      }
      contentKind = target;
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Conversion failed", "error");
    } finally {
      switching = false;
    }
  }

  function selectedFiles(): void {
    attachmentSelection = [...(files?.files ?? [])]
      .map(file => `${file.name}:${file.size}:${file.lastModified}`).join("|");
  }

  async function submit(): Promise<void> {
    const canonicalLanguage = contentKind === "text" ? normalizeLanguage(language) : "plaintext";
    if (!canonicalLanguage) {
      showNotice("Choose a supported language.", "error");
      return;
    }
    const submittedContent = contentKind === "rich_text" ? richHtml : content;
    if (new TextEncoder().encode(submittedContent).length > $appState.config.max_content_size_bytes) {
      showNotice(`Content exceeds the ${Math.floor($appState.config.max_content_size_bytes / 1024)} KiB server limit.`, "error");
      return;
    }
    const selected = [...(files?.files ?? [])];
    if (selected.length + (paste?.attachments.length ?? 0) > $appState.config.max_attachments_per_paste) {
      showNotice(`A paste can have at most ${$appState.config.max_attachments_per_paste} attachments.`, "error");
      return;
    }
    if (selected.reduce((size, file) => size + file.size, 0) > $appState.config.max_attachment_size_bytes) {
      showNotice("Selected attachments exceed the server upload limit.", "error");
      return;
    }
    submitting = true;
    let created: Paste | undefined;
    try {
      const body = {
        title,
        body: contentKind === "rich_text"
          ? { format: "rich_text", content: richHtml }
          : { format: "text", content, language: canonicalLanguage },
        visibility,
        ...(pasteId || (expirationMode !== "never" && expiresAt)
          ? { expires_at: expirationMode !== "never" && expiresAt ? new Date(expiresAt).toISOString() : null }
          : {}),
        ...(pasteId || readLimit
          ? { read_limit: readLimit ? Number(readLimit) : null }
          : {}),
        ...(canOrganize && (pasteId || folderId)
          ? { folder_id: folderId ? Number(folderId) : null }
          : {})
      };
      if (pasteId) {
        created = await updatePaste(pasteId, body as UpdatePasteInput, paste?._etag ?? "*");
      } else if (selected.length) {
        const flat: FlatCreateInput = {
          title, format: contentKind, content: submittedContent, visibility,
          ...(contentKind === "text" ? { language: canonicalLanguage } : {}),
          ...(expirationMode !== "never" && expiresAt ? { expires_at: new Date(expiresAt).toISOString() } : {}),
          ...(readLimit ? { read_limit: Number(readLimit) } : {}),
          ...(canOrganize && folderId ? { folder_id: Number(folderId) } : {})
        };
        created = await createPasteWithAttachments(flat, selected, crypto.randomUUID());
      } else {
        created = await createPaste(body as CreatePasteInput, crypto.randomUUID());
      }
      if (pasteId && selected.length) {
        try {
          await uploadAttachments(created.id, selected, created._etag ?? "*");
        } catch (reason) {
          paste = created;
          baseline = snapshot("");
          showNotice(
            `Paste changes were saved, but attachments were not uploaded: ${reason instanceof Error ? reason.message : "upload failed"}`,
            "error"
          );
          return;
        }
      }
      initialized = false;
      clearUnsavedChangesGuard();
      await navigate(`/pastes/${created.id}`);
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to save paste", "error");
    } finally {
      submitting = false;
    }
  }

  async function deletePaste(): Promise<void> {
    if (!pasteId || !confirm("Delete this paste permanently?")) return;
    try {
      await deletePasteRequest(pasteId, paste?._etag ?? "*");
      initialized = false;
      clearUnsavedChangesGuard();
      await navigate("/pastes");
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to delete paste", "error");
    }
  }
</script>

<ConversionDialog bind:this={conversionDialog}/>
{#if loading}
  <p class="muted">Loading paste…</p>
{:else if error}
  <section class="empty"><h1>Unable to edit this paste</h1><p>{error}</p><Link class="button" href="/pastes">Return to pastes</Link></section>
{:else}
  <section class="editor">
    <div class="page-heading"><div><p class="eyebrow">{paste ? "Edit" : "Create"}</p><h1>{paste ? pasteDisplayTitle(paste) : "New paste"}</h1></div></div>
    <form onsubmit={(event) => { event.preventDefault(); void submit(); }}>
      <label class="title-field"><span>Title</span><input bind:value={title} maxlength={$appState.config.max_title_characters} placeholder="Optional title"/></label>
      {#if contentKind === "rich_text"}
        <div class="content-field"><span>Content</span>
          <div class="content-editor content-editor-rich" style={`height:${editorHeight}px`}
            use:trackEditorResize>
            {#await import("../components/RichTextEditor.svelte") then module}
              {@const RichTextEditor = module.default}
              <RichTextEditor bind:document bind:html={richHtml}/>
            {/await}
          </div>
        </div>
      {:else}
        <div class="content-field"><span>Content</span>
          <div class="content-editor content-editor-text" style={`height:${editorHeight}px`}
            use:trackEditorResize>
            <CodeEditor bind:value={content} bind:language maxLength={$appState.config.max_content_size_bytes}/>
          </div>
        </div>
      {/if}
      <div class:without-folder={!canOrganize} class="form-grid">
        <label class="type-field"><span>Type</span><select value={contentKind} disabled={switching} onchange={changeKind}>
          {#each $appState.config.formats as format}<option value={format}>{format === "rich_text" ? "Rich text" : "Text"}</option>{/each}
        </select></label>
        <LanguagePicker bind:value={language} disabled={contentKind !== "text"}/>
        {#if canOrganize}
          <label class="folder-field"><span>Folder</span><select bind:value={folderId}>
            <option value="">Uncategorized</option>
            {#each folders as folder}<option value={String(folder.id)}>{folder.name}</option>{/each}
          </select></label>
        {/if}
        <label class="visibility-field"><span>Visibility</span><select bind:value={visibility}>
          {#each $appState.config.visibility_modes as mode}<option value={mode}>{mode}</option>{/each}
        </select></label>
        <label class="expiration-mode-field"><span>Expiration</span><select value={expirationMode} onchange={changeExpirationMode}>
          <option value="never">Never</option><option value="1h">1 hour</option>
          <option value="12h">12 hours</option><option value="1d">1 day</option>
          <option value="1w">1 week</option><option value="30d">30 days</option>
          <option value="1y">1 year</option><option value="custom">Custom…</option>
        </select></label>
        <label class="expiration-time-field"><span>Date and time</span>
          {#if expirationMode === "never"}
            <input type="text" value="Not applicable" disabled/>
          {:else}
            <input type="datetime-local" bind:value={expiresAt} required oninput={customizeExpiration}/>
          {/if}
        </label>
        <label class="read-limit-field"><span>Read limit</span><input type="number" min="1" bind:value={readLimit} placeholder="Unlimited"/></label>
      </div>
      {#if paste?.attachments.length}
        <div class="existing-attachments"><span>Current attachments</span>
          <AttachmentList pasteId={paste.id} attachments={paste.attachments} canDelete editing etag={paste._etag}
            ondelete={(attachment, etag) => { if (paste) paste = { ...paste, _etag: etag ?? paste._etag, attachments: paste.attachments.filter(item => item.id !== attachment.id) }; }}/>
          <small>Deleting an existing attachment takes effect immediately, even if you cancel editing.</small>
        </div>
      {/if}
      {#if $appState.config.attachments_enabled}
        <label><span>Add attachments</span><input bind:this={files} type="file" multiple onchange={selectedFiles}/>
          <small>Up to {$appState.config.max_attachments_per_paste} files; combined upload limit: {Math.floor($appState.config.max_attachment_size_bytes / 1024 / 1024)} MiB</small></label>
      {/if}
      <div class="actions">
        <button class="button primary" type="submit" disabled={submitting || switching}>{submitting ? "Saving…" : paste ? "Save changes" : "Create paste"}</button>
        <Link class="button" href={paste ? `/pastes/${paste.id}` : "/pastes"}>Cancel</Link>
        {#if paste}<button class="button danger" type="button" onclick={deletePaste}>Delete</button>{/if}
      </div>
    </form>
  </section>
{/if}
