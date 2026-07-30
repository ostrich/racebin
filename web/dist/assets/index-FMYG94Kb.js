(function(){const e=document.createElement("link").relList;if(e&&e.supports&&e.supports("modulepreload"))return;for(const n of document.querySelectorAll('link[rel="modulepreload"]'))s(n);new MutationObserver(n=>{for(const r of n)if(r.type==="childList")for(const p of r.addedNodes)p.tagName==="LINK"&&p.rel==="modulepreload"&&s(p)}).observe(document,{childList:!0,subtree:!0});function a(n){const r={};return n.integrity&&(r.integrity=n.integrity),n.referrerPolicy&&(r.referrerPolicy=n.referrerPolicy),n.crossOrigin==="use-credentials"?r.credentials="include":n.crossOrigin==="anonymous"?r.credentials="omit":r.credentials="same-origin",r}function s(n){if(n.ep)return;n.ep=!0;const r=a(n);fetch(n.href,r)}})();/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const O=(t,e,a=[])=>{const s=document.createElementNS("http://www.w3.org/2000/svg",t);return Object.keys(e).forEach(n=>{s.setAttribute(n,String(e[n]))}),a.length&&a.forEach(n=>{const r=O(...n);s.appendChild(r)}),s};var H=([t,e,a])=>O(t,e,a);/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const U=t=>Array.from(t.attributes).reduce((e,a)=>(e[a.name]=a.value,e),{}),V=t=>typeof t=="string"?t:!t||!t.class?"":t.class&&typeof t.class=="string"?t.class.split(" "):t.class&&Array.isArray(t.class)?t.class:"",z=t=>t.flatMap(V).map(a=>a.trim()).filter(Boolean).filter((a,s,n)=>n.indexOf(a)===s).join(" "),F=t=>t.replace(/(\w)(\w*)(_|-|\s*)/g,(e,a,s)=>a.toUpperCase()+s.toLowerCase()),I=(t,{nameAttr:e,icons:a,attrs:s})=>{var h;const n=t.getAttribute(e);if(n==null)return;const r=F(n),p=a[r];if(!p)return console.warn(`${t.outerHTML} icon name was not found in the provided icons object.`);const b=U(t),[w,y,v]=p,x={...y,"data-lucide":n,...s,...b},o=z(["lucide",`lucide-${n}`,b,s]);o&&Object.assign(x,{class:o});const $=H([w,x,v]);return(h=t.parentNode)==null?void 0:h.replaceChild($,t)};/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const f={xmlns:"http://www.w3.org/2000/svg",width:24,height:24,viewBox:"0 0 24 24",fill:"none",stroke:"currentColor","stroke-width":2,"stroke-linecap":"round","stroke-linejoin":"round"};/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const J=["svg",f,[["rect",{width:"14",height:"14",x:"8",y:"8",rx:"2",ry:"2"}],["path",{d:"M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const B=["svg",f,[["path",{d:"M15 3h6v6"}],["path",{d:"M10 14 21 3"}],["path",{d:"M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const K=["svg",f,[["path",{d:"M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"}],["path",{d:"M14 2v4a2 2 0 0 0 2 2h4"}],["path",{d:"M10 9H8"}],["path",{d:"M16 13H8"}],["path",{d:"M16 17H8"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const W=["svg",f,[["path",{d:"M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z"}],["circle",{cx:"16.5",cy:"7.5",r:".5",fill:"currentColor"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const X=["svg",f,[["path",{d:"M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"}],["polyline",{points:"10 17 15 12 10 7"}],["line",{x1:"15",x2:"3",y1:"12",y2:"12"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const Z=["svg",f,[["path",{d:"M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"}],["polyline",{points:"16 17 21 12 16 7"}],["line",{x1:"21",x2:"9",y1:"12",y2:"12"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const G=["svg",f,[["path",{d:"M12 20h9"}],["path",{d:"M16.376 3.622a1 1 0 0 1 3.002 3.002L7.368 18.635a2 2 0 0 1-.855.506l-2.872.838a.5.5 0 0 1-.62-.62l.838-2.872a2 2 0 0 1 .506-.854z"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const Q=["svg",f,[["path",{d:"M5 12h14"}],["path",{d:"M12 5v14"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const Y=["svg",f,[["circle",{cx:"11",cy:"11",r:"8"}],["path",{d:"m21 21-4.3-4.3"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const ee=["svg",f,[["path",{d:"M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"}],["circle",{cx:"12",cy:"12",r:"3"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const q=["svg",f,[["path",{d:"M3 6h18"}],["path",{d:"M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"}],["path",{d:"M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"}],["line",{x1:"10",x2:"10",y1:"11",y2:"17"}],["line",{x1:"14",x2:"14",y1:"11",y2:"17"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const te=["svg",f,[["circle",{cx:"12",cy:"8",r:"5"}],["path",{d:"M20 21a8 8 0 0 0-16 0"}]]];/**
 * @license lucide v0.468.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const R=({icons:t={},nameAttr:e="data-lucide",attrs:a={}}={})=>{if(!Object.values(t).length)throw new Error(`Please provide an icons object.
If you want to use all the icons you can import it like:
 \`import { createIcons, icons } from 'lucide';
lucide.createIcons({icons});\``);if(typeof document>"u")throw new Error("`createIcons()` only works in a browser environment.");const s=document.querySelectorAll(`[${e}]`);if(Array.from(s).forEach(n=>I(n,{nameAttr:e,icons:t,attrs:a})),e==="data-lucide"){const n=document.querySelectorAll("[icon-name]");n.length>0&&(console.warn("[Lucide] Some icons were found with the now deprecated icon-name attribute. These will still be replaced for backwards compatibility, but will no longer be supported in v1.0 and you should switch to data-lucide"),Array.from(n).forEach(r=>I(r,{nameAttr:"icon-name",icons:t,attrs:a})))}},ae=document.querySelector("#app");let l={authenticated:!1},k={name:"Racebin",max_file_size:0,file_uploads:!0,qr:!1};class se extends Error{constructor(e,a){super(a),this.status=e}}async function c(t,e={}){var n;const a=new Headers(e.headers);e.body&&!(e.body instanceof FormData)&&a.set("Content-Type","application/json"),l.csrf_token&&e.method&&e.method!=="GET"&&a.set("X-CSRF-Token",l.csrf_token);const s=await fetch(`/api/v2${t}`,{...e,headers:a});if(!s.ok){const r=await s.json().catch(()=>({error:{message:s.statusText}}));throw new se(s.status,((n=r.error)==null?void 0:n.message)??s.statusText)}return s.status===204?void 0:s.json()}const i=t=>String(t??"").replace(/[&<>"']/g,e=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"})[e]),S=t=>t?new Intl.DateTimeFormat(void 0,{dateStyle:"medium",timeStyle:"short"}).format(t*1e3):"Never",A=t=>t.title||t.slug,T=(t,e)=>`<button class="icon-button" type="button" title="${i(e)}" aria-label="${i(e)}" data-action="${t}"><i data-lucide="${t}"></i></button>`,d=(t,e)=>String(t.get(e)??"");function m(t){const e=l.user;ae.innerHTML=`
    <header>
      <a class="brand" href="/" data-link>${i(k.name)}</a>
      <nav>
        <a href="/explore" data-link>Explore</a>
        ${e?'<a href="/pastes" data-link>My pastes</a><a href="/new" data-link><i data-lucide="plus"></i> New</a>':""}
        ${(e==null?void 0:e.role)==="admin"?'<a href="/admin" data-link>Admin</a>':""}
      </nav>
      <div class="session">
        ${e?`<a href="/account" data-link><i data-lucide="user-round"></i><span>${i(e.username)}</span></a>${T("log-out","Log out")}`:'<a href="/login" data-link><i data-lucide="log-in"></i><span>Log in</span></a>'}
      </div>
    </header>
    <main>${t}</main>
    <div id="toast" role="status" aria-live="polite"></div>`,R({icons:{Copy:J,Edit3:G,ExternalLink:B,FileText:K,KeyRound:W,LogIn:X,LogOut:Z,Plus:Q,Search:Y,Settings:ee,Trash2:q,UserRound:te}})}function g(t,e=""){const a=document.querySelector("#toast");a&&(a.textContent=t,a.className=`show ${e}`,window.setTimeout(()=>a.className="",3500))}async function P(){var t;[l,k]=await Promise.all([c("/session").catch(()=>({authenticated:!1})),c("/config")]),(t=l.user)!=null&&t.force_password_change&&location.pathname!=="/account/password"&&history.replaceState({},"","/account/password")}function u(t){history.pushState({},"",t),C()}function ne(t){const e=t instanceof Error?t.message:"The request failed.";m(`<section class="empty"><h1>Unable to load this page</h1><p>${i(e)}</p><a class="button" href="/" data-link>Return home</a></section>`)}async function C(){const t=location.pathname;try{if(t==="/")return await oe();if(t==="/explore")return await M(!1);if(t==="/login")return ce();if(t==="/new")return L();if(t==="/pastes")return await M(!0);if(t==="/account")return await j();if(t==="/account/password")return de();if(t==="/admin")return pe();if(t==="/admin/pastes")return await ue();if(t==="/guide")return ie();if(t.startsWith("/invite/"))return le(t.slice(8));const e=t.match(/^\/pastes\/([^/]+)\/edit$/);if(e!=null&&e[1])return await L(e[1]);const a=t.match(/^\/pastes\/([^/]+)$/);if(a!=null&&a[1])return await re(a[1]);m('<section class="empty"><h1>Page not found</h1><p>The requested page does not exist.</p></section>')}catch(e){ne(e)}}function ie(){m(`<section><div class="page-heading"><div><p class="eyebrow">Reference</p><h1>API guide</h1></div><a class="button" href="/api/v2/openapi.json">OpenAPI JSON</a></div>
    <section class="panel"><h2>Authentication</h2><p>Use <code>Authorization: Bearer rbk_…</code>. Browser requests use the secure session cookie and send <code>X-CSRF-Token</code> for mutations.</p>
    <h2>Paste example</h2><pre class="content"><code>curl -H "Authorization: Bearer $RACEBIN_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"title":"Example","content":"Hello","syntax":"none","access":"unlisted"}' \\
  https://example.com/api/v2/pastes</code></pre>
    <p>All errors use <code>{"error":{"code":"…","message":"…"}}</code>. Timestamps are Unix seconds.</p></section></section>`)}async function oe(){if(l.user)return L();const t=await c("/pastes?access=public&page_size=8");m(`
    <section class="welcome"><div><p class="eyebrow">Share text and files</p><h1>${i(k.name)}</h1><p>Public pastes are open to everyone. Sign in to create and manage your own.</p>
    <div class="actions"><a class="button primary" href="/explore" data-link>Explore pastes</a><a class="button" href="/login" data-link>Log in</a></div></div></section>
    <section><div class="section-heading"><h2>Recently shared</h2><a href="/explore" data-link>View all</a></div>${_(t.items)}</section>`)}function _(t,e=!1){return t.length?`<div class="paste-list">${t.map(a=>`
    <article class="paste-row">
      <div class="paste-main"><a class="paste-title" href="/pastes/${i(a.slug)}" data-link>${i(A(a))}</a>
      <p>${i(a.content.slice(0,160).replace(/\s+/g," "))}</p></div>
      <div class="paste-meta"><span>${i(a.syntax)}</span><span>${i(a.access)}</span><time>${S(a.created)}</time></div>
      <div class="row-actions">
        ${T("copy","Copy link")}
        ${e?`<a class="icon-button" title="Edit" aria-label="Edit" href="/pastes/${i(a.slug)}/edit" data-link><i data-lucide="edit-3"></i></a>${T("trash-2","Delete")}`:""}
      </div><input type="hidden" value="${i(a.slug)}">
    </article>`).join("")}</div>`:'<div class="empty compact"><p>No pastes found.</p></div>'}function D(t){const e=Math.max(1,Math.ceil(t.total/t.page_size));if(e===1)return"";const a=(s,n)=>{const r=new URLSearchParams(location.search);return r.set("page",String(s)),`<a class="button" href="${location.pathname}?${r}" data-link>${n}</a>`};return`<nav class="pagination" aria-label="Pagination">
    ${t.page>1?a(t.page-1,"Previous"):""}
    <span>Page ${t.page} of ${e}</span>
    ${t.page<e?a(t.page+1,"Next"):""}
  </nav>`}async function M(t){if(t&&!l.user)return u("/login");const e=new URLSearchParams(location.search);e.set("page_size","50"),t&&e.set("mine","true"),t||e.set("access","public");const a=await c(`/pastes?${e}`);m(`<section>
    <div class="page-heading"><div><p class="eyebrow">${t?"Workspace":"Public"}</p><h1>${t?"My pastes":"Explore"}</h1></div>${t?'<a class="button primary" href="/new" data-link><i data-lucide="plus"></i> New paste</a>':""}</div>
    <form class="filters" id="paste-filters"><label><span>Search</span><input name="search" value="${i(e.get("search")??"")}" placeholder="Title, content, or ID"></label>
    <label><span>Access</span><select name="access"><option value="">All access</option>${["public","unlisted","owner"].map(s=>`<option ${e.get("access")===s?"selected":""}>${s}</option>`).join("")}</select></label>
    <button class="button" type="submit"><i data-lucide="search"></i> Filter</button></form>
    <p class="result-count">${a.total} paste${a.total===1?"":"s"}</p>${_(a.items,t)}${D(a)}</section>`)}async function re(t){const e=await c(`/pastes/${encodeURIComponent(t)}/consume`),a=l.user&&(l.user.id===e.owner_user_id||l.user.role==="admin");if(e.kind==="url"){m(`<section class="empty"><p class="eyebrow">Short link</p><h1>${i(A(e))}</h1><p>Redirecting…</p></section>`),location.replace(e.content);return}m(`<article class="paste-view">
    <div class="page-heading"><div><p class="eyebrow">${i(e.access)} · ${i(e.syntax)}</p><h1>${i(A(e))}</h1></div>
    <div class="actions"><a class="button" href="/api/v2/pastes/${i(e.slug)}/raw">Raw</a>${e.files.length?`<a class="button" href="/api/v2/pastes/${i(e.slug)}/archive">ZIP</a>`:""}${k.qr?`<a class="button" href="/api/v2/pastes/${i(e.slug)}/qr">QR</a>`:""}${a?`<a class="button primary" href="/pastes/${i(e.slug)}/edit" data-link><i data-lucide="edit-3"></i> Edit</a>`:""}</div></div>
    <pre class="content"><code>${i(e.content)}</code></pre>
    ${e.files.length?`<section><h2>Files</h2><div class="files">${e.files.map(s=>`<div class="file-row" data-file-id="${s.id}" data-slug="${i(e.slug)}"><a href="/api/v2/pastes/${i(e.slug)}/files/${s.id}"><i data-lucide="file-text"></i><span>${i(s.name)}</span><small>${s.size.toLocaleString()} bytes</small></a>${a?'<button class="icon-button" type="button" title="Delete file" aria-label="Delete file" data-action="delete-file"><i data-lucide="trash-2"></i></button>':""}</div>`).join("")}</div></section>`:""}
    <footer class="paste-stats"><span>Created ${S(e.created)}</span><span>Expires ${S(e.expiration)}</span><span>${e.read_count} reads</span></footer>
  </article>`)}async function L(t){if(!l.user)return u("/login");const e=t?await c(`/pastes/${encodeURIComponent(t)}`):void 0;m(`<section class="editor">
    <div class="page-heading"><div><p class="eyebrow">${e?"Edit":"Create"}</p><h1>${e?i(A(e)):"New paste"}</h1></div></div>
    <form id="paste-form">
      <label class="title-field"><span>Title</span><input name="title" maxlength="200" value="${i((e==null?void 0:e.title)??"")}" placeholder="Optional title"></label>
      <label><span>Content</span><textarea name="content" spellcheck="false">${i((e==null?void 0:e.content)??"")}</textarea></label>
      <div class="form-grid">
        <label><span>Type</span><select name="kind"><option value="text">Text</option><option value="url" ${(e==null?void 0:e.kind)==="url"?"selected":""}>URL</option></select></label>
        <label><span>Syntax</span><select name="syntax">${["none","auto","sh","c","cpp","cs","go","html","java","js","json","kt","lua","php","py","r","rb","rs","swift","xml","yaml"].map(a=>`<option ${(e==null?void 0:e.syntax)===a?"selected":""}>${a}</option>`).join("")}</select></label>
        <label><span>Access</span><select name="access">${["public","unlisted","owner"].map(a=>`<option ${(e==null?void 0:e.access)===a||!e&&a==="unlisted"?"selected":""}>${a}</option>`).join("")}</select></label>
        <label><span>Expires</span><input type="datetime-local" name="expiration" value="${e!=null&&e.expiration?new Date(e.expiration*1e3).toISOString().slice(0,16):""}"></label>
        <label><span>Burn after reads</span><input type="number" min="0" name="burn_after_reads" value="${(e==null?void 0:e.burn_after_reads)??0}"></label>
      </div>
      ${k.file_uploads?`<label><span>Add files</span><input type="file" name="files" multiple><small>Combined upload limit: ${Math.floor(k.max_file_size/1024/1024)} MiB</small></label>`:""}
      <div class="actions"><button class="button primary" type="submit">${e?"Save changes":"Create paste"}</button>${e?'<button class="button danger" type="button" data-action="delete-paste">Delete</button>':""}</div>
      <input type="hidden" name="slug" value="${i(t??"")}">
    </form></section>`)}function ce(){m(`<section class="auth"><form id="login-form"><p class="eyebrow">Account</p><h1>Log in</h1>
    <label><span>Username</span><input name="username" autocomplete="username" required autofocus></label>
    <label><span>Password</span><input type="password" name="password" autocomplete="current-password" required></label>
    <label class="check"><input type="checkbox" name="remember"><span>Keep me signed in</span></label>
    <button class="button primary" type="submit">Log in</button></form></section>`)}function le(t){m(`<section class="auth"><form id="invite-form"><p class="eyebrow">Invitation</p><h1>Create your account</h1>
    <label><span>Username</span><input name="username" autocomplete="username" required></label>
    <label><span>Password</span><input type="password" name="password" minlength="12" autocomplete="new-password" required></label>
    <button class="button primary" type="submit">Create account</button><input type="hidden" name="token" value="${i(t)}"></form></section>`)}async function j(){if(!l.user)return u("/login");const t=await c("/account/api-keys");m(`<section><div class="page-heading"><div><p class="eyebrow">Settings</p><h1>Account</h1></div><a class="button" href="/account/password" data-link>Change password</a></div>
    <section class="panel"><h2>API keys</h2><p class="muted">Tokens are shown once when created.</p>
      <div class="key-list">${t.length?t.map(e=>`<div class="key-row"><div><strong>${i(e.name)}</strong><code>rbk_${i(e.prefix)}_...</code><small>${i(e.scopes)}</small></div><label class="switch"><input type="checkbox" data-key="${e.id}" ${e.enabled?"checked":""}><span></span></label>${T("trash-2","Delete API key")}<input type="hidden" value="${e.id}"></div>`).join(""):'<p class="empty compact">No API keys.</p>'}</div>
      <form id="key-form" class="key-form"><label><span>Name</span><input name="name" required maxlength="100"></label>
      <fieldset><legend>Scopes</legend>${["paste:read","paste:write","paste:delete","paste:list"].map(e=>`<label class="check"><input type="checkbox" name="scopes" value="${e}"><span>${e}</span></label>`).join("")}</fieldset>
      <button class="button primary" type="submit"><i data-lucide="key-round"></i> Create key</button></form>
    </section></section>`)}function de(){if(!l.user)return u("/login");m(`<section class="auth"><form id="password-form"><p class="eyebrow">Security</p><h1>Change password</h1>
    <label><span>Current password</span><input type="password" name="current_password" autocomplete="current-password" required></label>
    <label><span>New password</span><input type="password" name="new_password" minlength="12" autocomplete="new-password" required></label>
    <button class="button primary" type="submit">Update password</button></form></section>`)}function pe(){var t;if(((t=l.user)==null?void 0:t.role)!=="admin")return u("/");m(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>Admin</h1></div></div>
    <div class="admin-links"><a href="/admin/pastes" data-link><i data-lucide="file-text"></i><div><strong>All pastes</strong><span>Search, filter, edit, and remove pastes</span></div></a>
    <button type="button" data-action="admin-users"><i data-lucide="user-round"></i><div><strong>Users</strong><span>Roles and account access</span></div></button>
    <button type="button" data-action="admin-invites"><i data-lucide="plus"></i><div><strong>Invitations</strong><span>Create and revoke invitation links</span></div></button>
    <button type="button" data-action="admin-keys"><i data-lucide="key-round"></i><div><strong>API keys</strong><span>Review and revoke keys</span></div></button></div>
    <section id="admin-detail" class="panel hidden"></section></section>`)}async function ue(){var a;if(((a=l.user)==null?void 0:a.role)!=="admin")return u("/");const t=new URLSearchParams(location.search);t.set("page_size","100");const e=await c(`/admin/pastes?${t}`);m(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>All pastes</h1></div><a class="button" href="/admin" data-link>Admin home</a></div>
    <form class="filters admin-filters" id="paste-filters"><label><span>Search</span><input name="search" value="${i(t.get("search")??"")}"></label>
    <label><span>Access</span><select name="access"><option value="">All</option>${["public","unlisted","owner"].map(s=>`<option ${t.get("access")===s?"selected":""}>${s}</option>`).join("")}</select></label>
    <label><span>Owner ID</span><input type="number" name="owner_user_id" value="${i(t.get("owner_user_id")??"")}"></label><button class="button" type="submit">Filter</button></form>
    <p class="result-count">${e.total} pastes</p>${_(e.items,!0)}${D(e)}</section>`)}document.addEventListener("click",async t=>{var n,r,p,b,w,y,v,x;const e=t.target,a=e.closest("a[data-link]");if(a){t.preventDefault(),u(a.pathname+a.search);return}const s=(n=e.closest("[data-action]"))==null?void 0:n.dataset.action;try{if(s==="log-out"&&(await c("/session",{method:"DELETE"}),l={authenticated:!1},u("/")),s==="copy"){const o=(p=(r=e.closest(".paste-row"))==null?void 0:r.querySelector('input[type="hidden"]'))==null?void 0:p.value;o&&(await navigator.clipboard.writeText(`${location.origin}/pastes/${o}`),g("Link copied."))}if(s==="trash-2"){const o=e.closest(".paste-row"),$=(b=o==null?void 0:o.querySelector('input[type="hidden"]'))==null?void 0:b.value,h=e.closest(".key-row"),N=(w=h==null?void 0:h.querySelector('input[type="hidden"]'))==null?void 0:w.value;$&&confirm("Delete this paste permanently?")&&(await c(`/pastes/${$}`,{method:"DELETE"}),o==null||o.remove()),N&&confirm("Delete this API key permanently?")&&(await c(`/account/api-keys/${N}`,{method:"DELETE"}),h==null||h.remove())}if(s==="delete-paste"){const o=(y=document.querySelector('input[name="slug"]'))==null?void 0:y.value;o&&confirm("Delete this paste permanently?")&&(await c(`/pastes/${o}`,{method:"DELETE"}),u("/pastes"))}if(s==="delete-file"){const o=e.closest(".file-row"),$=o==null?void 0:o.dataset.slug,h=o==null?void 0:o.dataset.fileId;$&&h&&confirm("Delete this file permanently?")&&(await c(`/pastes/${encodeURIComponent($)}/files/${h}`,{method:"DELETE"}),o.remove())}if(s==="create-invite"){const o=await c("/admin/invites",{method:"POST"});await navigator.clipboard.writeText(`${location.origin}${o.url}`),g("Invitation link copied."),await E("invites")}if(s==="revoke-invite"){const o=(v=e.closest("[data-id]"))==null?void 0:v.dataset.id;o&&(await c(`/admin/invites/${o}`,{method:"DELETE"}),await E("invites"))}if(s==="delete-admin-key"){const o=(x=e.closest("[data-id]"))==null?void 0:x.dataset.id;o&&confirm("Delete this API key permanently?")&&(await c(`/admin/api-keys/${o}`,{method:"DELETE"}),await E("keys"))}s!=null&&s.startsWith("admin-")&&await E(s.slice(6))}catch(o){g(o instanceof Error?o.message:"Request failed","error")}});document.addEventListener("change",async t=>{const e=t.target;if(e.dataset.userEnabled){try{await c(`/admin/users/${e.dataset.userEnabled}`,{method:"PATCH",body:JSON.stringify({enabled:e.checked})})}catch(a){e.checked=!e.checked,g(a instanceof Error?a.message:"Request failed","error")}return}if(e.dataset.adminKey){try{await c(`/admin/api-keys/${e.dataset.adminKey}`,{method:"PATCH",body:JSON.stringify({enabled:e.checked})})}catch(a){e.checked=!e.checked,g(a instanceof Error?a.message:"Request failed","error")}return}if(e.dataset.userRole){try{await c(`/admin/users/${e.dataset.userRole}`,{method:"PATCH",body:JSON.stringify({role:e.value})})}catch(a){g(a instanceof Error?a.message:"Request failed","error"),await E("users")}return}if(e.dataset.key)try{await c(`/account/api-keys/${e.dataset.key}`,{method:"PATCH",body:JSON.stringify({enabled:e.checked})})}catch(a){e.checked=!e.checked,g(a instanceof Error?a.message:"Request failed","error")}});document.addEventListener("submit",async t=>{t.preventDefault();const e=t.target,a=new FormData(e),s=[...e.querySelectorAll("button, input[type=submit]")];s.forEach(n=>n.disabled=!0);try{if(e.id==="login-form"&&(await c("/session",{method:"POST",body:JSON.stringify({username:d(a,"username"),password:d(a,"password"),remember:a.has("remember")})}),await P(),u("/pastes")),e.id==="invite-form"&&(await c(`/invites/${encodeURIComponent(d(a,"token"))}/accept`,{method:"POST",body:JSON.stringify({username:d(a,"username"),password:d(a,"password")})}),await P(),u("/pastes")),e.id==="paste-form"){const n=d(a,"expiration"),r={title:d(a,"title"),content:d(a,"content"),kind:d(a,"kind"),syntax:d(a,"syntax"),access:d(a,"access"),expiration:n?Math.floor(new Date(n).getTime()/1e3):null,burn_after_reads:Number(d(a,"burn_after_reads")||0)},p=d(a,"slug"),b=p?await c(`/pastes/${p}`,{method:"PATCH",body:JSON.stringify(r)}):await c("/pastes",{method:"POST",body:JSON.stringify(r)}),w=a.getAll("files").filter(y=>y instanceof File&&y.size>0);if(w.length){const y=new FormData;w.forEach(v=>y.append("files",v));try{await c(`/pastes/${b.slug}/files`,{method:"POST",body:y})}catch(v){throw p||await c(`/pastes/${b.slug}`,{method:"DELETE"}).catch(()=>{}),v}}u(`/pastes/${b.slug}`)}if(e.id==="password-form"&&(await c("/account/password",{method:"PATCH",body:JSON.stringify({current_password:d(a,"current_password"),new_password:d(a,"new_password")})}),l={authenticated:!1},u("/login")),e.id==="key-form"){const n=await c("/account/api-keys",{method:"POST",body:JSON.stringify({name:d(a,"name"),scopes:a.getAll("scopes")})});prompt("API key created. Store it now; it will not be shown again.",n.token),await j()}if(e.id==="paste-filters"){const n=new URLSearchParams;a.forEach((r,p)=>{r&&n.set(p,String(r))}),u(`${location.pathname}?${n}`)}}catch(n){g(n instanceof Error?n.message:"Request failed","error")}finally{s.forEach(n=>n.disabled=!1)}});async function E(t){const e=document.querySelector("#admin-detail");if(e.classList.remove("hidden"),t==="users"){const a=await c("/admin/users");e.innerHTML=`<h2>Users</h2><div class="table">${a.map(s=>`<div><strong>${i(s.username)}</strong><select data-user-role="${s.id}"><option value="user" ${s.role==="user"?"selected":""}>User</option><option value="admin" ${s.role==="admin"?"selected":""}>Admin</option></select><label class="check"><input type="checkbox" data-user-enabled="${s.id}" ${s.enabled?"checked":""}><span>Enabled</span></label></div>`).join("")}</div>`}else if(t==="invites"){const a=await c("/admin/invites");e.innerHTML=`<div class="section-heading"><h2>Invitations</h2><button class="button primary" data-action="create-invite">Create invitation</button></div><div class="table">${a.map(s=>`<div data-id="${s.id}"><code>${i(s.token_prefix)}…</code><span>${i(s.status)} · ${S(s.expires)}</span>${s.status==="Active"?'<button class="button" data-action="revoke-invite">Revoke</button>':"<span></span>"}</div>`).join("")}</div>`}else{const a=await c("/admin/api-keys");e.innerHTML=`<h2>API keys</h2><div class="table">${a.map(s=>`<div data-id="${s.id}"><div><strong>${i(s.name)}</strong><br><code>${i(s.prefix)}</code></div><label class="check"><input type="checkbox" data-admin-key="${s.id}" ${s.enabled?"checked":""}><span>Enabled</span></label><button class="icon-button" title="Delete API key" aria-label="Delete API key" data-action="delete-admin-key"><i data-lucide="trash-2"></i></button></div>`).join("")}</div>`}R({icons:{Trash2:q}})}window.addEventListener("popstate",()=>void C());P().then(C);
