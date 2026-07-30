<script lang="ts">
  import { onMount } from "svelte";
  import AccountPage from "./pages/AccountPage.svelte";
  import AdminPage from "./pages/AdminPage.svelte";
  import AdminPastesPage from "./pages/AdminPastesPage.svelte";
  import GuidePage from "./pages/GuidePage.svelte";
  import HomePage from "./pages/HomePage.svelte";
  import InvitationPage from "./pages/InvitationPage.svelte";
  import LoginPage from "./pages/LoginPage.svelte";
  import PasswordPage from "./pages/PasswordPage.svelte";
  import PasteFormPage from "./pages/PasteFormPage.svelte";
  import PasteListPage from "./pages/PasteListPage.svelte";
  import PasteViewPage from "./pages/PasteViewPage.svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  import Link from "./components/Link.svelte";
  import Shell from "./components/Shell.svelte";
  import {
    deferRouteReady,
    hasUnsavedChanges,
    initializeRouter,
    locationState,
    navigate,
    setDiscardPrompt
  } from "./router";
  import { loadSession } from "./session";
  import { appState } from "./state";

  let discardDialog: ConfirmDialog;
  let startupError = $state("");

  onMount(() => {
    setDiscardPrompt(() => discardDialog.ask({
      title: "Discard unsaved changes?",
      message: "Your changes will not be saved.",
      confirmLabel: "Discard changes",
      dangerous: true
    }));
    const stopRouter = initializeRouter();
    const startupReady = deferRouteReady();
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (!hasUnsavedChanges()) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", beforeUnload);
    void loadSession()
      .then(async () => {
        const user = $appState.session.user;
        if (user?.password_change_required && location.pathname !== "/account/password") {
          await navigate("/account/password", { replace: true });
        }
      })
      .catch(error => { startupError = error instanceof Error ? error.message : "Unable to start Racebin"; })
      .finally(startupReady);
    return () => {
      stopRouter();
      window.removeEventListener("beforeunload", beforeUnload);
    };
  });

  let routeKey = $derived(`${$locationState.path}?${$locationState.query}`);
  let authenticated = $derived(Boolean($appState.session.user));
  let administrator = $derived($appState.session.user?.role === "admin");
  $effect(() => {
    if (!$appState.ready) return;
    const route = $locationState.route;
    const protectedRoute = [
      "new-paste", "my-pastes", "edit-paste", "account", "password"
    ].includes(route.name);
    if (!authenticated && protectedRoute) void navigate("/login", { replace: true });
    else if (authenticated && route.name === "login") void navigate("/pastes", { replace: true });
    else if (!administrator && (route.name === "admin" || route.name === "admin-pastes")) {
      void navigate("/", { replace: true });
    }
  });
</script>

<ConfirmDialog bind:this={discardDialog}/>
<Shell>
  {#if startupError}
    <section class="empty"><h1>Unable to load Racebin</h1><p>{startupError}</p></section>
  {:else if !$appState.ready}
    <p class="muted">Loading Racebin…</p>
  {:else}
    {#key routeKey}
      {@const route = $locationState.route}
      {#if route.name === "home"}
        {#if authenticated}<PasteFormPage/>{:else}<HomePage/>{/if}
      {:else if route.name === "explore"}
        <PasteListPage mine={false} query={$locationState.query}/>
      {:else if route.name === "login"}
        {#if authenticated}<PasteListPage mine query={new URLSearchParams()}/>{:else}<LoginPage/>{/if}
      {:else if route.name === "new-paste"}
        {#if authenticated}<PasteFormPage/>{:else}<LoginPage/>{/if}
      {:else if route.name === "my-pastes"}
        {#if authenticated}<PasteListPage mine query={$locationState.query}/>{:else}<LoginPage/>{/if}
      {:else if route.name === "paste"}
        <PasteViewPage pasteId={route.pasteId}/>
      {:else if route.name === "edit-paste"}
        {#if authenticated}<PasteFormPage pasteId={route.pasteId}/>{:else}<LoginPage/>{/if}
      {:else if route.name === "account"}
        {#if authenticated}<AccountPage/>{:else}<LoginPage/>{/if}
      {:else if route.name === "password"}
        {#if authenticated}<PasswordPage/>{:else}<LoginPage/>{/if}
      {:else if route.name === "admin"}
        {#if administrator}<AdminPage/>{:else}<section class="empty"><h1>Access denied</h1><Link class="button" href="/">Return home</Link></section>{/if}
      {:else if route.name === "admin-pastes"}
        {#if administrator}<AdminPastesPage query={$locationState.query}/>{:else}<section class="empty"><h1>Access denied</h1><Link class="button" href="/">Return home</Link></section>{/if}
      {:else if route.name === "guide"}
        <GuidePage/>
      {:else if route.name === "invitation"}
        <InvitationPage token={route.token}/>
      {:else}
        <section class="empty"><h1>Page not found</h1><p>The requested page does not exist.</p><Link class="button" href="/">Return home</Link></section>
      {/if}
    {/key}
  {/if}
</Shell>
