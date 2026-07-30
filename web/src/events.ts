import { requestApi } from "./api";
import { normalizeLanguage } from "./highlighting";
import { navigate } from "./router";
import { loadSession } from "./session";
import { state } from "./state";
import type { Paste } from "./types";
import { formValue, showNotice } from "./ui";
import { accountView } from "./views/account";
import { loadAdmin } from "./views/admin_detail";

document.addEventListener("click", async event => {
  const target = event.target as HTMLElement;
  const link = target.closest<HTMLAnchorElement>("a[data-link]");
  if (link) { event.preventDefault(); navigate(link.pathname + link.search); return; }
  const action = target.closest<HTMLElement>("[data-action]")?.dataset.action;
  try {
    if (action === "log-out") { await requestApi("/session", { method: "DELETE" }); state.session = { authenticated: false }; navigate("/"); }
    if (action === "copy") {
      const pasteId = target.closest<HTMLElement>(".paste-row")?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      if (pasteId) { await navigator.clipboard.writeText(`${location.origin}/pastes/${pasteId}`); showNotice("Link copied."); }
    }
    if (action === "copy-content") {
      const content = document.querySelector<HTMLElement>("#paste-code")?.textContent;
      if (content !== undefined) {
        await navigator.clipboard.writeText(content);
        showNotice("Paste copied.");
      }
    }
    if (action === "trash-2") {
      const row = target.closest<HTMLElement>(".paste-row");
      const pasteId = row?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      const keyRow = target.closest<HTMLElement>(".key-row");
      const key = keyRow?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      if (pasteId && confirm("Delete this paste permanently?")) { await requestApi(`/pastes/${pasteId}`, { method: "DELETE" }); row?.remove(); }
      if (key && confirm("Delete this API key permanently?")) { await requestApi(`/account/api-keys/${key}`, { method: "DELETE" }); keyRow?.remove(); }
    }
    if (action === "delete-paste") {
      const pasteId = (document.querySelector<HTMLInputElement>('input[name="pasteId"]'))?.value;
      if (pasteId && confirm("Delete this paste permanently?")) { await requestApi(`/pastes/${pasteId}`, { method: "DELETE" }); navigate("/pastes"); }
    }
    if (action === "delete-attachment") {
      const row = target.closest<HTMLElement>(".attachment-row");
      const pasteId = row?.dataset.pasteId;
      const attachmentId = row?.dataset.attachmentId;
      if (pasteId && attachmentId && confirm("Delete this attachment permanently?")) {
        await requestApi(`/pastes/${encodeURIComponent(pasteId)}/attachments/${attachmentId}`, { method: "DELETE" });
        row.remove();
      }
    }
    if (action === "create-invitation") {
      const invitation = await requestApi<{url:string}>("/admin/invitations", { method: "POST" });
      await navigator.clipboard.writeText(`${location.origin}${invitation.url}`);
      showNotice("Invitation link copied.");
      await loadAdmin("invitations");
    }
    if (action === "revoke-invitation") {
      const id = target.closest<HTMLElement>("[data-id]")?.dataset.id;
      if (id) { await requestApi(`/admin/invitations/${id}`, { method: "DELETE" }); await loadAdmin("invitations"); }
    }
    if (action === "delete-admin-key") {
      const id = target.closest<HTMLElement>("[data-id]")?.dataset.id;
      if (id && confirm("Delete this API key permanently?")) { await requestApi(`/admin/api-keys/${id}`, { method: "DELETE" }); await loadAdmin("keys"); }
    }
    if (action?.startsWith("admin-")) await loadAdmin(action.slice(6));
  } catch (error) { showNotice(error instanceof Error ? error.message : "Request failed", "error"); }
});

document.addEventListener("change", async event => {
  const input = event.target as HTMLInputElement;
  if (input.dataset.userEnabled) {
    try { await requestApi(`/admin/users/${input.dataset.userEnabled}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
    catch (error) { input.checked = !input.checked; showNotice(error instanceof Error ? error.message : "Request failed", "error"); }
    return;
  }
  if (input.dataset.adminKey) {
    try { await requestApi(`/admin/api-keys/${input.dataset.adminKey}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
    catch (error) { input.checked = !input.checked; showNotice(error instanceof Error ? error.message : "Request failed", "error"); }
    return;
  }
  if (input.dataset.userRole) {
    try { await requestApi(`/admin/users/${input.dataset.userRole}`, { method: "PATCH", body: JSON.stringify({ role: input.value }) }); }
    catch (error) { showNotice(error instanceof Error ? error.message : "Request failed", "error"); await loadAdmin("users"); }
    return;
  }
  if (!input.dataset.key) return;
  try { await requestApi(`/account/api-keys/${input.dataset.key}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
  catch (error) { input.checked = !input.checked; showNotice(error instanceof Error ? error.message : "Request failed", "error"); }
});

document.addEventListener("submit", async event => {
  event.preventDefault();
  const form = event.target as HTMLFormElement;
  const data = new FormData(form);
  const controls = [...form.querySelectorAll<HTMLButtonElement | HTMLInputElement>("button, input[type=submit]")];
  controls.forEach(control => control.disabled = true);
  try {
    if (form.id === "login-form") {
      await requestApi("/session", { method: "POST", body: JSON.stringify({ username: formValue(data,"username"), password: formValue(data,"password"), remember: data.has("remember") }) });
      await loadSession(); navigate("/pastes");
    }
    if (form.id === "invitation-form") {
      await requestApi(`/invitations/${encodeURIComponent(formValue(data,"token"))}/redeem`, { method: "POST", body: JSON.stringify({ username: formValue(data,"username"), password: formValue(data,"password") }) });
      await loadSession(); navigate("/pastes");
    }
    if (form.id === "paste-form") {
      const expiresAt = formValue(data,"expires_at");
      const languageInput = form.elements.namedItem("language") as HTMLInputElement;
      const language = normalizeLanguage(formValue(data,"language"));
      if (!language) {
        languageInput.setCustomValidity("Choose a supported language.");
        languageInput.reportValidity();
        throw new Error("Choose a supported language.");
      }
      languageInput.setCustomValidity("");
      const body = {
        title: formValue(data,"title"), content: formValue(data,"content"), content_kind: formValue(data,"content_kind"),
        language, visibility: formValue(data,"visibility"),
        expires_at: expiresAt ? Math.floor(new Date(expiresAt).getTime()/1000) : null,
        read_limit: formValue(data,"read_limit") ? Number(formValue(data,"read_limit")) : null
      };
      const pasteId = formValue(data,"pasteId");
      const paste = pasteId ? await requestApi<Paste>(`/pastes/${pasteId}`, { method: "PATCH", body: JSON.stringify(body) }) : await requestApi<Paste>("/pastes", { method: "POST", body: JSON.stringify(body) });
      const files = data.getAll("attachments").filter(value => value instanceof File && value.size > 0);
      if (files.length) {
        const upload = new FormData();
        files.forEach(file => upload.append("attachments", file));
        try {
          await requestApi(`/pastes/${paste.id}/attachments`, { method: "POST", body: upload });
        } catch (error) {
          if (!pasteId) await requestApi(`/pastes/${paste.id}`, { method: "DELETE" }).catch(() => undefined);
          throw error;
        }
      }
      navigate(`/pastes/${paste.id}`);
    }
    if (form.id === "password-form") {
      await requestApi("/account/password", { method: "PATCH", body: JSON.stringify({ current_password: formValue(data,"current_password"), new_password: formValue(data,"new_password") }) });
      state.session = { authenticated:false }; navigate("/login");
    }
    if (form.id === "key-form") {
      const result = await requestApi<{token:string}>("/account/api-keys", { method: "POST", body: JSON.stringify({ name: formValue(data,"name"), scopes: data.getAll("scopes") }) });
      prompt("API key created. Store it now; it will not be shown again.", result.token); await accountView();
    }
    if (form.id === "paste-filters") {
      const params = new URLSearchParams();
      data.forEach((value,key) => { if (value) params.set(key,String(value)); });
      navigate(`${location.pathname}?${params}`);
    }
  } catch (error) {
    showNotice(error instanceof Error ? error.message : "Request failed", "error");
  } finally {
    controls.forEach(control => control.disabled = false);
  }
});
