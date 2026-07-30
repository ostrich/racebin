import { requestApi } from "../api";
import {
  connectHighlightedEditor,
  connectLanguagePicker,
  highlightElement,
  languageMenu,
  normalizeLanguage,
  updateLineNumbers
} from "../highlighting";
import { navigate } from "../router";
import { setUnsavedChangesGuard } from "../navigation_guard";
import { state } from "../state";
import type { Page, Paste, RichTextDocument } from "../types";
import { formatDate, escapeHtml, iconButton, renderLayout, pasteDisplayTitle } from "../ui";

function pasteFormatLabel(paste: Paste): string {
  if (paste.content_kind === "rich_text") return "Rich text";
  if (paste.content_kind === "redirect") return "Redirect";
  return paste.language;
}

export async function home(): Promise<void> {
  if (state.session.user) return pasteForm();
  const page = await requestApi<Page<Paste>>("/pastes?visibility=public&page_size=8");
  renderLayout(`
    <section class="welcome"><div><p class="eyebrow">Code, notes, and files—shared simply</p><h1>${escapeHtml(state.config.site_name)}</h1><p>Browse public pastes below, or sign in to create syntax-highlighted and rich-text pastes of your own.</p>
    <div class="actions"><a class="button primary" href="/explore" data-link>Explore pastes</a><a class="button" href="/login" data-link>Log in</a></div></div></section>
    <section><div class="section-heading"><h2>Recently shared</h2><a href="/explore" data-link>View all</a></div>${pasteRows(page.items)}</section>`);
}

export function pasteRows(items: Paste[], manage = false): string {
  if (!items.length) return `<div class="empty compact"><p>No pastes found.</p></div>`;
  return `<div class="paste-list">${items.map(paste => `
    <article class="paste-row">
      <div class="paste-main"><a class="paste-title" href="/pastes/${escapeHtml(paste.id)}" data-link>${escapeHtml(pasteDisplayTitle(paste))}</a>
      <p>${escapeHtml(paste.content.slice(0, 160).replace(/\s+/g, " "))}</p></div>
      <div class="paste-meta"><span>${escapeHtml(pasteFormatLabel(paste))}</span><span>${escapeHtml(paste.visibility)}</span><time>${formatDate(paste.created_at)}</time></div>
      <div class="row-actions">
        ${iconButton("copy", "Copy link")}
        ${manage ? `<a class="icon-button" title="Edit" aria-label="Edit" href="/pastes/${escapeHtml(paste.id)}/edit" data-link><i data-icon="edit-3"></i></a>${iconButton("trash-2", "Delete")}` : ""}
      </div><input type="hidden" value="${escapeHtml(paste.id)}">
    </article>`).join("")}</div>`;
}

export function pagination(page: Page<unknown>): string {
  const pages = Math.max(1, Math.ceil(page.total_items / page.page_size));
  if (pages === 1) return "";
  const link = (number: number, label: string) => {
    const params = new URLSearchParams(location.search);
    params.set("page", String(number));
    return `<a class="button" href="${location.pathname}?${params}" data-link>${label}</a>`;
  };
  return `<nav class="pagination" aria-label="Pagination">
    ${page.page > 1 ? link(page.page - 1, "Previous") : ""}
    <span>Page ${page.page} of ${pages}</span>
    ${page.page < pages ? link(page.page + 1, "Next") : ""}
  </nav>`;
}

export async function pasteList(mine: boolean): Promise<void> {
  if (mine && !state.session.user) return navigate("/login");
  const params = new URLSearchParams(location.search);
  params.set("page_size", "50");
  if (mine) params.set("mine", "true");
  if (!mine) params.set("visibility", "public");
  const page = await requestApi<Page<Paste>>(`/pastes?${params}`);
  renderLayout(`<section>
    <div class="page-heading"><div><p class="eyebrow">${mine ? "Workspace" : "Public"}</p><h1>${mine ? "My pastes" : "Explore"}</h1></div>${mine ? `<a class="button primary" href="/pastes/new" data-link><i data-icon="plus"></i> New paste</a>` : ""}</div>
    <form class="filters" id="paste-filters"><label><span>Search</span><input name="search" value="${escapeHtml(params.get("search") ?? "")}" placeholder="Title, content, or ID"></label>
    <label><span>Visibility</span><select name="visibility"><option value="">All visibility</option>${["public","unlisted","private"].map(value => `<option ${params.get("visibility") === value ? "selected" : ""}>${value}</option>`).join("")}</select></label>
    <button class="button" type="submit"><i data-icon="search"></i> Filter</button></form>
    <p class="result-count">${page.total_items} paste${page.total_items === 1 ? "" : "s"}</p>${pasteRows(page.items, mine)}${pagination(page)}</section>`);
}

export async function pasteView(pasteId: string): Promise<void> {
  const paste = await requestApi<Paste>(`/pastes/${encodeURIComponent(pasteId)}/consume`);
  const own = state.session.user && (state.session.user.id === paste.owner_id || state.session.user.role === "admin");
  if (paste.content_kind === "redirect") {
    renderLayout(`<section class="empty"><p class="eyebrow">Short link</p><h1>${escapeHtml(pasteDisplayTitle(paste))}</h1><p>Redirecting…</p></section>`);
    location.replace(paste.content);
    return;
  }
  renderLayout(`<article class="paste-view">
    <div class="page-heading"><div><p class="eyebrow">${escapeHtml(paste.visibility)} · ${escapeHtml(pasteFormatLabel(paste))}</p><h1>${escapeHtml(pasteDisplayTitle(paste))}</h1></div>
    <div class="actions"><a class="button" href="/api/v1/pastes/${escapeHtml(paste.id)}/raw">Raw</a><button class="button" type="button" data-action="copy-content"><i data-icon="copy"></i> Copy</button>${paste.attachments.length ? `<a class="button" href="/api/v1/pastes/${escapeHtml(paste.id)}/archive">ZIP</a>` : ""}${state.config.qr_codes_enabled ? `<a class="button" href="/api/v1/pastes/${escapeHtml(paste.id)}/qr">QR</a>` : ""}${own ? `<a class="button primary" href="/pastes/${escapeHtml(paste.id)}/edit" data-link><i data-icon="edit-3"></i> Edit</a>` : ""}</div></div>
    ${paste.content_kind === "rich_text"
      ? `<div id="rich-text-viewer" class="rich-text-viewer"></div>`
      : `<div class="paste-code"><div id="paste-lines" class="line-numbers" aria-hidden="true"></div><pre class="content"><code id="paste-code">${escapeHtml(paste.content)}</code></pre></div>`}
    <textarea id="paste-plain-content" hidden>${escapeHtml(paste.content)}</textarea>
    ${paste.attachments.length ? `<section><h2>Attachments</h2><div class="attachments">${paste.attachments.map(attachment => `<div class="attachment-row" data-attachment-id="${attachment.id}" data-paste-id="${escapeHtml(paste.id)}"><a href="/api/v1/pastes/${escapeHtml(paste.id)}/attachments/${attachment.id}"><i data-icon="file-text"></i><span>${escapeHtml(attachment.filename)}</span><small>${attachment.size_bytes.toLocaleString()} bytes</small></a>${own ? `<button class="icon-button" type="button" title="Delete attachment" aria-label="Delete attachment" data-action="delete-attachment"><i data-icon="trash-2"></i></button>` : ""}</div>`).join("")}</div></section>` : ""}
    <footer class="paste-stats"><span>Created ${formatDate(paste.created_at)}</span><span>Expires ${formatDate(paste.expires_at)}</span><span>${paste.read_count} reads</span></footer>
  </article>`);
  if (paste.content_kind === "rich_text" && paste.document) {
    const { mountRichTextViewer } = await import("../rich_text_editor");
    const viewer = document.querySelector<HTMLElement>("#rich-text-viewer");
    if (viewer) mountRichTextViewer(viewer, paste.document);
  }
  const code = document.querySelector<HTMLElement>("#paste-code");
  const lines = document.querySelector<HTMLElement>("#paste-lines");
  if (code) await highlightElement(code, paste.content, paste.language);
  if (lines) updateLineNumbers(lines, paste.content);
}

export async function pasteForm(pasteId?: string): Promise<void> {
  if (!state.session.user) return navigate("/login");
  const paste = pasteId ? await requestApi<Paste>(`/pastes/${encodeURIComponent(pasteId)}`) : undefined;
  const selectedLanguage = normalizeLanguage(paste?.language ?? "auto") ?? "auto";
  renderLayout(`<section class="editor">
    <div class="page-heading"><div><p class="eyebrow">${paste ? "Edit" : "Create"}</p><h1>${paste ? escapeHtml(pasteDisplayTitle(paste)) : "New paste"}</h1></div></div>
    <form id="paste-form">
      <label class="title-field"><span>Title</span><input name="title" maxlength="200" value="${escapeHtml(paste?.title ?? "")}" placeholder="Optional title"></label>
      <label id="text-content-field" class="content-field"><span>Content</span><div class="code-editor">
        <div id="editor-lines" class="line-numbers" aria-hidden="true"></div>
        <pre aria-hidden="true"><code id="editor-highlight" class="hljs"></code></pre>
        <textarea name="content" spellcheck="false" aria-label="Paste content">${escapeHtml(paste?.content ?? "")}</textarea>
      </div></label>
      <div id="rich-content-field" class="content-field hidden"><span>Content</span>
        <div class="rich-text-toolbar" role="toolbar" aria-label="Rich-text formatting">
          <select id="rich-block-type" aria-label="Block type">
            <option value="paragraph">Paragraph</option><option value="heading-1">Heading 1</option>
            <option value="heading-2">Heading 2</option><option value="heading-3">Heading 3</option>
          </select>
          ${[
            ["bold","Bold"],["italic","Italic"],["underline","Underline"],["strike","Strike"],
            ["link","Link"],["bullet-list","Bulleted list"],["ordered-list","Numbered list"],
            ["blockquote","Quote"],["code","Inline code"],["code-block","Code block"],
            ["horizontal-rule","Separator"],["align-left","Align left"],["align-center","Align center"],
            ["align-right","Align right"],["clear-formatting","Clear formatting"],
            ["undo","Undo"],["redo","Redo"]
          ].map(([command,label]) => `<button type="button" data-rich-command="${command}" title="${label}" aria-label="${label}">${label}</button>`).join("")}
        </div>
        <div id="rich-text-editor" class="rich-text-editor"></div>
      </div>
      <div class="form-grid">
        <label><span>Type</span><select id="content-kind" name="content_kind" data-current-kind="${paste?.content_kind ?? "text"}"><option value="text">Text</option><option value="rich_text" ${paste?.content_kind === "rich_text" ? "selected" : ""}>Rich text</option><option value="redirect" ${paste?.content_kind === "redirect" ? "selected" : ""}>Redirect</option></select></label>
        <div id="language-field" class="language-field"><label for="language-input">Language <small>Type to filter languages.</small></label><div class="language-picker">
          <input id="language-input" name="language" value="${escapeHtml(selectedLanguage)}" autocomplete="off" role="combobox" aria-autocomplete="list" aria-expanded="false" aria-controls="language-options-menu" placeholder="Type or choose">
          <div id="language-options-menu" class="language-options" role="listbox" hidden>${languageMenu()}</div>
        </div></div>
        <label><span>Visibility</span><select name="visibility">${["public","unlisted","private"].map(value => `<option ${paste?.visibility === value || (!paste && value === "unlisted") ? "selected" : ""}>${value}</option>`).join("")}</select></label>
        <label><span>Expires</span><input type="datetime-local" name="expires_at" value="${paste?.expires_at ? new Date(paste.expires_at * 1000).toISOString().slice(0,16) : ""}"></label>
        <label><span>Read limit</span><input type="number" min="1" name="read_limit" value="${paste?.read_limit ?? ""}" placeholder="Unlimited"></label>
      </div>
      ${state.config.attachments_enabled ? `<label><span>Add attachments</span><input type="file" name="attachments" multiple><small>Combined upload limit: ${Math.floor(state.config.max_attachment_size_bytes / 1024 / 1024)} MiB</small></label>` : ""}
      <div class="actions"><button class="button primary" type="submit">${paste ? "Save changes" : "Create paste"}</button><a class="button" href="${paste ? `/pastes/${escapeHtml(paste.id)}` : "/pastes"}" data-link>Cancel</a>${paste ? `<button class="button danger" type="button" data-action="delete-paste">Delete</button>` : ""}</div>
      <input type="hidden" name="pasteId" value="${escapeHtml(pasteId ?? "")}">
    </form></section>`);
  const textarea = document.querySelector<HTMLTextAreaElement>('#paste-form textarea[name="content"]');
  const output = document.querySelector<HTMLElement>("#editor-highlight");
  const lines = document.querySelector<HTMLElement>("#editor-lines");
  const languageInput = document.querySelector<HTMLInputElement>("#language-input");
  const languageOptions = document.querySelector<HTMLElement>("#language-options-menu");
  if (textarea && output && languageInput && lines) {
    connectHighlightedEditor(textarea, output, languageInput, lines);
  }
  if (languageInput && languageOptions) connectLanguagePicker(languageInput, languageOptions);
  const getRichDocument = await connectContentKindSelector(paste);
  const form = document.querySelector<HTMLFormElement>("#paste-form");
  if (form) {
    const initialState = pasteFormState(form, getRichDocument);
    setUnsavedChangesGuard(() => pasteFormState(form, getRichDocument) !== initialState);
  }
}

type Conversion = { content: string; document: RichTextDocument | null };

function pasteFormState(
  form: HTMLFormElement,
  getRichDocument: () => RichTextDocument | undefined
): string {
  const fields = [...new FormData(form).entries()]
    .filter(([, value]) => !(value instanceof File && !value.name && value.size === 0))
    .map(([name, value]) => [
      name,
      value instanceof File
        ? { name: value.name, size: value.size, last_modified: value.lastModified }
        : value
    ]);
  const kind = form.elements.namedItem("content_kind") as HTMLSelectElement | null;
  return JSON.stringify({
    fields,
    document: kind?.value === "rich_text" ? getRichDocument() : null
  });
}

async function convertContent(
  sourceKind: "text" | "rich_text",
  targetKind: "text" | "rich_text",
  content: string,
  document: RichTextDocument | null
): Promise<Conversion> {
  return requestApi<Conversion>("/pastes/convert", {
    method: "POST",
    body: JSON.stringify({
      source_kind: sourceKind,
      target_kind: targetKind,
      content,
      document
    })
  });
}

async function confirmConversion(targetKind: string, preview: string): Promise<boolean> {
  const dialog = document.createElement("dialog");
  dialog.className = "conversion-dialog";
  dialog.innerHTML = `<div><h2>Convert to ${escapeHtml(targetKind.replace("_", " "))}?</h2>
    <p class="muted">${targetKind === "text" ? "Formatting will be removed when you save." : "Review the converted text before continuing."}</p>
    <pre>${escapeHtml(preview.slice(0, 4000))}</pre>
    <div class="actions"><button class="button" type="button" data-conversion="cancel">Cancel</button><button class="button primary" type="button" data-conversion="confirm">Convert</button></div></div>`;
  document.body.append(dialog);
  dialog.showModal();
  return new Promise(resolve => {
    const finish = (confirmed: boolean) => {
      dialog.close();
      dialog.remove();
      resolve(confirmed);
    };
    dialog.addEventListener("click", event => {
      const choice = (event.target as HTMLElement).closest<HTMLElement>("[data-conversion]")
        ?.dataset.conversion;
      if (choice) finish(choice === "confirm");
    });
    dialog.addEventListener("cancel", event => {
      event.preventDefault();
      finish(false);
    }, { once: true });
  });
}

async function connectContentKindSelector(
  paste?: Paste
): Promise<() => RichTextDocument | undefined> {
  const selector = document.querySelector<HTMLSelectElement>("#content-kind");
  const textarea = document.querySelector<HTMLTextAreaElement>('#paste-form textarea[name="content"]');
  const textField = document.querySelector<HTMLElement>("#text-content-field");
  const richField = document.querySelector<HTMLElement>("#rich-content-field");
  const richElement = document.querySelector<HTMLElement>("#rich-text-editor");
  const languageField = document.querySelector<HTMLElement>("#language-field");
  const languageInput = document.querySelector<HTMLInputElement>("#language-input");
  if (
    !selector || !textarea || !textField || !richField || !richElement
    || !languageField || !languageInput
  ) return () => undefined;

  const richModule = await import("../rich_text_editor");
  let richDocument = paste?.document ?? null;
  let richPlaintext = paste?.content ?? "";
  const drafts = new Map<string, string>([
    [paste?.content_kind ?? "text", paste?.content ?? ""]
  ]);

  const showKind = (kind: string) => {
    const rich = kind === "rich_text";
    richField.classList.toggle("hidden", !rich);
    textField.classList.toggle("hidden", rich);
    languageField.classList.toggle("hidden", kind !== "text");
    languageInput.disabled = kind !== "text";
    selector.dataset.currentKind = kind;
    if (rich && richDocument) {
      richModule.mountRichTextEditor(richElement, richDocument);
    }
  };

  if (paste?.content_kind === "rich_text" && richDocument) showKind("rich_text");
  else showKind(paste?.content_kind ?? "text");

  selector.addEventListener("change", async () => {
    const sourceKind = (selector.dataset.currentKind ?? "text") as "text" | "rich_text" | "redirect";
    const targetKind = selector.value as "text" | "rich_text" | "redirect";
    if (sourceKind === targetKind) return;
    selector.disabled = true;
    try {
      if (sourceKind === "rich_text") {
        richDocument = richModule.richTextDocument() ?? richDocument;
        const converted = await convertContent("rich_text", "text", richPlaintext, richDocument);
        richPlaintext = converted.content;
        if (
          converted.content.length > 0 &&
          !(await confirmConversion(targetKind, converted.content))
        ) {
          selector.value = sourceKind;
          return;
        }
        drafts.set(targetKind, drafts.get(targetKind) ?? converted.content);
        textarea.value = drafts.get(targetKind)!;
      } else if (targetKind === "rich_text") {
        drafts.set(sourceKind, textarea.value);
        const source = textarea.value;
        if (!richDocument || source !== richPlaintext) {
          const converted = await convertContent("text", "rich_text", source, null);
          richDocument = converted.document;
          richPlaintext = converted.content;
        }
        if (
          richPlaintext.length > 0 &&
          !(await confirmConversion(targetKind, richPlaintext))
        ) {
          selector.value = sourceKind;
          return;
        }
      } else {
        drafts.set(sourceKind, textarea.value);
        if (!(await confirmConversion(targetKind, drafts.get(targetKind) ?? textarea.value))) {
          selector.value = sourceKind;
          return;
        }
        textarea.value = drafts.get(targetKind) ?? textarea.value;
      }
      showKind(targetKind);
    } catch (error) {
      selector.value = sourceKind;
      throw error;
    } finally {
      selector.disabled = false;
    }
  });
  return () => richModule.richTextDocument();
}
