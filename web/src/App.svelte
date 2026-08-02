<script lang="ts">
  import { onMount } from "svelte";
  import AccountPage from "./pages/AccountPage.svelte";
  import AdminPage from "./pages/AdminPage.svelte";
  import AdminPastesPage from "./pages/AdminPastesPage.svelte";
  import AdminUserPage from "./pages/AdminUserPage.svelte";
  import AdminUsersPage from "./pages/AdminUsersPage.svelte";
  import HelpPage from "./pages/HelpPage.svelte";
  import HomePage from "./pages/HomePage.svelte";
  import InvitationPage from "./pages/InvitationPage.svelte";
  import LoginPage from "./pages/LoginPage.svelte";
  import PasswordPage from "./pages/PasswordPage.svelte";
  import PasswordResetPage from "./pages/PasswordResetPage.svelte";
  import PasteFormPage from "./pages/PasteFormPage.svelte";
  import PasteListPage from "./pages/PasteListPage.svelte";
  import PasteViewPage from "./pages/PasteViewPage.svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  import Link from "./components/Link.svelte";
  import Shell from "./components/Shell.svelte";
  import {
    locationState,
    navigationReady,
    setDiscardPrompt,
    startNavigation
  } from "./navigation";
  import type { RouteLocation } from "./navigation";
  import { loadSession } from "./session";
  import { appState } from "./state";

  let discardDialog: ConfirmDialog;
  let startupError = $state("");

  function accessPolicy(location: RouteLocation): string | null {
    const user = $appState.session.user;
    const authenticated = Boolean(user);
    if (user?.password_change_required && location.route.name !== "password") {
      return "/account/password";
    }
    const protectedRoute = [
      "new-paste", "my-pastes", "edit-paste", "account", "password", "help"
    ].includes(location.route.name);
    if (!authenticated && protectedRoute) return "/login";
    if (authenticated && location.route.name === "login") return "/pastes";
    const adminRoute = ["admin", "admin-pastes", "admin-users", "admin-user"]
      .includes(location.route.name);
    if (user?.role !== "admin" && adminRoute) return "/";
    return null;
  }

  onMount(() => {
    setDiscardPrompt(() => discardDialog.ask({
      title: "Discard unsaved changes?",
      message: "Your changes will not be saved.",
      confirmLabel: "Discard changes",
      dangerous: true
    }));
    let stopNavigation: (() => void) | undefined;
    void loadSession()
      .then(async () => {
        stopNavigation = await startNavigation({
          accessPolicy,
          siteName: () => $appState.config.site_name
        });
      })
      .catch(error => { startupError = error instanceof Error ? error.message : "Unable to start Racebin"; })
    return () => stopNavigation?.();
  });

  let routeKey = $derived($locationState.path);
  let authenticated = $derived(Boolean($appState.session.user));
  let plainAnonymousHome = $derived(
    $appState.ready
      && $appState.config.plain_home_enabled
      && !authenticated
      && $locationState.route.name === "home"
  );
  let minimalShell = $derived(!$appState.ready || plainAnonymousHome);
</script>

<ConfirmDialog bind:this={discardDialog}/>
<Shell minimal={minimalShell}>
  {#if startupError}
    <section class="empty"><h1>Unable to load Racebin</h1><p>{startupError}</p></section>
  {:else if !$appState.ready || !$navigationReady}
    <p class="muted">Loading Racebin…</p>
  {:else}
    {#key routeKey}
      {@const route = $locationState.route}
      {#if route.name === "home"}
        {#if authenticated}
          <PasteFormPage/>
        {:else if $appState.config.plain_home_enabled}
          <LoginPage/>
        {:else}
          <HomePage/>
        {/if}
      {:else if route.name === "explore"}
        <PasteListPage mine={false} query={$locationState.query}/>
      {:else if route.name === "login"}
        <LoginPage/>
      {:else if route.name === "new-paste"}
        <PasteFormPage/>
      {:else if route.name === "my-pastes"}
        <PasteListPage mine query={$locationState.query}/>
      {:else if route.name === "paste"}
        <PasteViewPage pasteId={route.pasteId}/>
      {:else if route.name === "edit-paste"}
        <PasteFormPage pasteId={route.pasteId}/>
      {:else if route.name === "account"}
        <AccountPage/>
      {:else if route.name === "password"}
        <PasswordPage/>
      {:else if route.name === "admin"}
        <AdminPage/>
      {:else if route.name === "admin-pastes"}
        <AdminPastesPage query={$locationState.query}/>
      {:else if route.name === "admin-users"}
        <AdminUsersPage/>
      {:else if route.name === "admin-user"}
        <AdminUserPage userId={route.userId}/>
      {:else if route.name === "help"}
        <HelpPage/>
      {:else if route.name === "password-reset"}
        <PasswordResetPage token={route.token}/>
      {:else if route.name === "invitation"}
        <InvitationPage token={route.token}/>
      {:else}
        <section class="empty"><h1>Page not found</h1><p>The requested page does not exist.</p><Link class="button" href="/">Return home</Link></section>
      {/if}
    {/key}
  {/if}
</Shell>
