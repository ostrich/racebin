import { api } from "./api";
import { normalizeSyntax } from "./highlighting";
import { navigate } from "./router";
import { loadSession } from "./session";
import { state } from "./state";
import type { Paste } from "./types";
import { formValue, notice } from "./ui";
import { accountView } from "./views/account";
import { loadAdmin } from "./views/admin_detail";

document.addEventListener("click", async event => {
  const target = event.target as HTMLElement;
  const link = target.closest<HTMLAnchorElement>("a[data-link]");
  if (link) { event.preventDefault(); navigate(link.pathname + link.search); return; }
  const action = target.closest<HTMLElement>("[data-action]")?.dataset.action;
  try {
    if (action === "log-out") { await api("/session", { method: "DELETE" }); state.session = { authenticated: false }; navigate("/"); }
    if (action === "copy") {
      const slug = target.closest<HTMLElement>(".paste-row")?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      if (slug) { await navigator.clipboard.writeText(`${location.origin}/pastes/${slug}`); notice("Link copied."); }
    }
    if (action === "copy-content") {
      const content = document.querySelector<HTMLElement>("#paste-code")?.textContent;
      if (content !== undefined) {
        await navigator.clipboard.writeText(content);
        notice("Paste copied.");
      }
    }
    if (action === "trash-2") {
      const row = target.closest<HTMLElement>(".paste-row");
      const slug = row?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      const keyRow = target.closest<HTMLElement>(".key-row");
      const key = keyRow?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      if (slug && confirm("Delete this paste permanently?")) { await api(`/pastes/${slug}`, { method: "DELETE" }); row?.remove(); }
      if (key && confirm("Delete this API key permanently?")) { await api(`/account/api-keys/${key}`, { method: "DELETE" }); keyRow?.remove(); }
    }
    if (action === "delete-paste") {
      const slug = (document.querySelector<HTMLInputElement>('input[name="slug"]'))?.value;
      if (slug && confirm("Delete this paste permanently?")) { await api(`/pastes/${slug}`, { method: "DELETE" }); navigate("/pastes"); }
    }
    if (action === "delete-file") {
      const row = target.closest<HTMLElement>(".file-row");
      const slug = row?.dataset.slug;
      const fileId = row?.dataset.fileId;
      if (slug && fileId && confirm("Delete this file permanently?")) {
        await api(`/pastes/${encodeURIComponent(slug)}/files/${fileId}`, { method: "DELETE" });
        row.remove();
      }
    }
    if (action === "create-invite") {
      const invite = await api<{url:string}>("/admin/invites", { method: "POST" });
      await navigator.clipboard.writeText(`${location.origin}${invite.url}`);
      notice("Invitation link copied.");
      await loadAdmin("invites");
    }
    if (action === "revoke-invite") {
      const id = target.closest<HTMLElement>("[data-id]")?.dataset.id;
      if (id) { await api(`/admin/invites/${id}`, { method: "DELETE" }); await loadAdmin("invites"); }
    }
    if (action === "delete-admin-key") {
      const id = target.closest<HTMLElement>("[data-id]")?.dataset.id;
      if (id && confirm("Delete this API key permanently?")) { await api(`/admin/api-keys/${id}`, { method: "DELETE" }); await loadAdmin("keys"); }
    }
    if (action?.startsWith("admin-")) await loadAdmin(action.slice(6));
  } catch (error) { notice(error instanceof Error ? error.message : "Request failed", "error"); }
});

document.addEventListener("change", async event => {
  const input = event.target as HTMLInputElement;
  if (input.dataset.userEnabled) {
    try { await api(`/admin/users/${input.dataset.userEnabled}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
    catch (error) { input.checked = !input.checked; notice(error instanceof Error ? error.message : "Request failed", "error"); }
    return;
  }
  if (input.dataset.adminKey) {
    try { await api(`/admin/api-keys/${input.dataset.adminKey}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
    catch (error) { input.checked = !input.checked; notice(error instanceof Error ? error.message : "Request failed", "error"); }
    return;
  }
  if (input.dataset.userRole) {
    try { await api(`/admin/users/${input.dataset.userRole}`, { method: "PATCH", body: JSON.stringify({ role: input.value }) }); }
    catch (error) { notice(error instanceof Error ? error.message : "Request failed", "error"); await loadAdmin("users"); }
    return;
  }
  if (!input.dataset.key) return;
  try { await api(`/account/api-keys/${input.dataset.key}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
  catch (error) { input.checked = !input.checked; notice(error instanceof Error ? error.message : "Request failed", "error"); }
});

document.addEventListener("submit", async event => {
  event.preventDefault();
  const form = event.target as HTMLFormElement;
  const data = new FormData(form);
  const controls = [...form.querySelectorAll<HTMLButtonElement | HTMLInputElement>("button, input[type=submit]")];
  controls.forEach(control => control.disabled = true);
  try {
    if (form.id === "login-form") {
      await api("/session", { method: "POST", body: JSON.stringify({ username: formValue(data,"username"), password: formValue(data,"password"), remember: data.has("remember") }) });
      await loadSession(); navigate("/pastes");
    }
    if (form.id === "invite-form") {
      await api(`/invites/${encodeURIComponent(formValue(data,"token"))}/accept`, { method: "POST", body: JSON.stringify({ username: formValue(data,"username"), password: formValue(data,"password") }) });
      await loadSession(); navigate("/pastes");
    }
    if (form.id === "paste-form") {
      const expiration = formValue(data,"expiration");
      const syntaxInput = form.elements.namedItem("syntax") as HTMLInputElement;
      const syntax = normalizeSyntax(formValue(data,"syntax"));
      if (!syntax) {
        syntaxInput.setCustomValidity("Choose a supported language.");
        syntaxInput.reportValidity();
        throw new Error("Choose a supported syntax language.");
      }
      syntaxInput.setCustomValidity("");
      const body = {
        title: formValue(data,"title"), content: formValue(data,"content"), kind: formValue(data,"kind"),
        syntax, access: formValue(data,"access"),
        expiration: expiration ? Math.floor(new Date(expiration).getTime()/1000) : null,
        burn_after_reads: Number(formValue(data,"burn_after_reads") || 0)
      };
      const slug = formValue(data,"slug");
      const paste = slug ? await api<Paste>(`/pastes/${slug}`, { method: "PATCH", body: JSON.stringify(body) }) : await api<Paste>("/pastes", { method: "POST", body: JSON.stringify(body) });
      const files = data.getAll("files").filter(value => value instanceof File && value.size > 0);
      if (files.length) {
        const upload = new FormData();
        files.forEach(file => upload.append("files", file));
        try {
          await api(`/pastes/${paste.slug}/files`, { method: "POST", body: upload });
        } catch (error) {
          if (!slug) await api(`/pastes/${paste.slug}`, { method: "DELETE" }).catch(() => undefined);
          throw error;
        }
      }
      navigate(`/pastes/${paste.slug}`);
    }
    if (form.id === "password-form") {
      await api("/account/password", { method: "PATCH", body: JSON.stringify({ current_password: formValue(data,"current_password"), new_password: formValue(data,"new_password") }) });
      state.session = { authenticated:false }; navigate("/login");
    }
    if (form.id === "key-form") {
      const result = await api<{token:string}>("/account/api-keys", { method: "POST", body: JSON.stringify({ name: formValue(data,"name"), scopes: data.getAll("scopes") }) });
      prompt("API key created. Store it now; it will not be shown again.", result.token); await accountView();
    }
    if (form.id === "paste-filters") {
      const params = new URLSearchParams();
      data.forEach((value,key) => { if (value) params.set(key,String(value)); });
      navigate(`${location.pathname}?${params}`);
    }
  } catch (error) {
    notice(error instanceof Error ? error.message : "Request failed", "error");
  } finally {
    controls.forEach(control => control.disabled = false);
  }
});
