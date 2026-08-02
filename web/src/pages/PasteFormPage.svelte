<script lang="ts">
  import { onMount, tick } from "svelte";
  import { requestApi } from "../api";
  import AttachmentList from "../components/AttachmentList.svelte";
  import CodeEditor from "../components/CodeEditor.svelte";
  import ConversionDialog from "../components/ConversionDialog.svelte";
  import LanguagePicker from "../components/LanguagePicker.svelte";
  import Link from "../components/Link.svelte";
  import { pasteDisplayTitle } from "../format";
  import { normalizeLanguage } from "../highlighting";
  import { showNotice } from "../notices";
  import { deferRouteReady, guardUnsavedChanges, navigate } from "../router";
  import { appState } from "../state";
  import type { Folder, FolderOverview, Paste, RichTextDocument } from "../types";

  type ContentKind = "text" | "rich_text";
  type ExpirationMode = "never" | "1h" | "12h" | "1d" | "1w" | "30d" | "1y" | "custom";
  type Conversion = { body: { format: ContentKind; content: string; language?: string; plain_text?: string } };

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
  const initialLoadReady = deferRouteReady();
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

  function snapshot(selectedAttachments = attachmentSelection): string {
    return JSON.stringify({
      title, content, richHtml: contentKind === "rich_text" ? richHtml : null,
      contentKind, folderId, language: contentKind === "text" ? language : null,
      visibility, expirationMode, expiresAt, readLimit, attachmentSelection: selectedAttachments
    });
  }
  let dirty = $derived(initialized && snapshot() !== baseline);

  $effect(() => {
    guardUnsavedChanges(() => dirty);
    return () => guardUnsavedChanges();
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
    void requestApi<FolderOverview>("/folders")
      .then(result => { folders = result.items; })
      .catch(reason => showNotice(reason instanceof Error ? reason.message : "Unable to load folders", "error"));
    loading = Boolean(pasteId);
    if (!pasteId) {
      initialize();
      loading = false;
      return;
    }
    void requestApi<Paste>(`/pastes/${encodeURIComponent(pasteId)}/source`)
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
    return requestApi<Conversion>("/content-conversions", {
      method: "POST",
      invalidateQueries: false,
      body: JSON.stringify({
        source: sourceKind === "rich_text"
          ? { format: "rich_text", content: richHtml }
          : { format: "text", content, language },
        target_format: targetKind
      })
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
    submitting = true;
    let created: Paste | undefined;
    try {
      const body = {
        title,
        body: contentKind === "rich_text"
          ? { format: "rich_text", content: richHtml }
          : { format: "text", content, language: canonicalLanguage },
        visibility,
        expires_at: expirationMode !== "never" && expiresAt
          ? new Date(expiresAt).toISOString()
          : null,
        read_limit: readLimit ? Number(readLimit) : null,
        ...(canOrganize ? { folder_id: folderId ? Number(folderId) : null } : {})
      };
      const selected = [...(files?.files ?? [])];
      if (pasteId) {
        created = await requestApi<Paste>(`/pastes/${encodeURIComponent(pasteId)}`, {
          method: "PATCH",
          headers: { "If-Match": paste?._etag ?? "*" },
          body: JSON.stringify(body)
        });
      } else if (selected.length) {
        const upload = new FormData();
        upload.append("title", title);
        upload.append("format", contentKind);
        upload.append("content", contentKind === "rich_text" ? richHtml : content);
        if (contentKind === "text") upload.append("language", canonicalLanguage);
        upload.append("visibility", visibility);
        if (expirationMode !== "never" && expiresAt) {
          upload.append("expires_at", new Date(expiresAt).toISOString());
        }
        if (readLimit) upload.append("read_limit", readLimit);
        if (canOrganize && folderId) upload.append("folder_id", folderId);
        selected.forEach(file => upload.append("file", file));
        created = await requestApi<Paste>("/pastes", {
          method: "POST",
          headers: { "Idempotency-Key": crypto.randomUUID() },
          body: upload
        });
      } else {
        created = await requestApi<Paste>("/pastes", {
          method: "POST",
          headers: { "Idempotency-Key": crypto.randomUUID() },
          body: JSON.stringify(body)
        });
      }
      if (pasteId && selected.length) {
        const upload = new FormData();
        selected.forEach(file => upload.append("file", file));
        try {
          await requestApi(`/pastes/${encodeURIComponent(created.id)}/attachments`, {
            method: "POST",
            headers: { "If-Match": created._etag ?? "*" },
            body: upload
          });
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
      guardUnsavedChanges();
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
      await requestApi(`/pastes/${encodeURIComponent(pasteId)}`, {
        method: "DELETE", headers: { "If-Match": paste?._etag ?? "*" }
      });
      initialized = false;
      guardUnsavedChanges();
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
      <label class="title-field"><span>Title</span><input bind:value={title} maxlength="200" placeholder="Optional title"/></label>
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
            <CodeEditor bind:value={content} bind:language/>
          </div>
        </div>
      {/if}
      <div class:without-folder={!canOrganize} class="form-grid">
        <label class="type-field"><span>Type</span><select value={contentKind} disabled={switching} onchange={changeKind}>
          <option value="text">Text</option><option value="rich_text">Rich text</option>
        </select></label>
        <LanguagePicker bind:value={language} disabled={contentKind !== "text"}/>
        {#if canOrganize}
          <label class="folder-field"><span>Folder</span><select bind:value={folderId}>
            <option value="">Uncategorized</option>
            {#each folders as folder}<option value={String(folder.id)}>{folder.name}</option>{/each}
          </select></label>
        {/if}
        <label class="visibility-field"><span>Visibility</span><select bind:value={visibility}>
          <option value="public">public</option><option value="unlisted">unlisted</option><option value="private">private</option>
        </select></label>
        <label class="expiration-mode-field"><span>Expiration</span><select value={expirationMode} onchange={changeExpirationMode}>
          <option value="never">Never</option><option value="1h">1 hour</option>
          <option value="12h">12 hours</option><option value="1d">1 day</option>
          <option value="1w">1 week</option><option value="30d">30 days</option>
          <option value="1y">1 year</option><option value="custom">Custom…</option>
        </select></label>
        <label class="expiration-time-field"><span>Date and time</span><input type="datetime-local"
          bind:value={expiresAt} disabled={expirationMode === "never"} required={expirationMode !== "never"}
          oninput={customizeExpiration}/></label>
        <label class="read-limit-field"><span>Read limit</span><input type="number" min="1" bind:value={readLimit} placeholder="Unlimited"/></label>
      </div>
      {#if paste?.attachments.length}
        <div class="existing-attachments"><span>Current attachments</span>
          <AttachmentList pasteId={paste.id} attachments={paste.attachments} canDelete editing
            ondelete={(attachment) => { if (paste) paste.attachments = paste.attachments.filter(item => item.id !== attachment.id); }}/>
          <small>Deleting an existing attachment takes effect immediately, even if you cancel editing.</small>
        </div>
      {/if}
      {#if $appState.config.attachments_enabled}
        <label><span>Add attachments</span><input bind:this={files} type="file" multiple onchange={selectedFiles}/>
          <small>Combined upload limit: {Math.floor($appState.config.max_attachment_size_bytes / 1024 / 1024)} MiB</small></label>
      {/if}
      <div class="actions">
        <button class="button primary" type="submit" disabled={submitting || switching}>{submitting ? "Saving…" : paste ? "Save changes" : "Create paste"}</button>
        <Link class="button" href={paste ? `/pastes/${paste.id}` : "/pastes"}>Cancel</Link>
        {#if paste}<button class="button danger" type="button" onclick={deletePaste}>Delete</button>{/if}
      </div>
    </form>
  </section>
{/if}
