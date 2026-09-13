<script lang="ts">
  import { onMount } from "svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  import Shell from "./components/Shell.svelte";
  import { setConfirmationPrompt } from "./app/confirmations";
  import { loadSession } from "./app/session";
  import { appState } from "./app/state";
  import { loadRouteComponent, routeProps } from "./navigation/components";
  import {
    holdNavigation,
    locationState,
    navigationReady,
    routeAccess,
    setDiscardPrompt,
    startNavigation
  } from "./navigation";
  import type { Route, RouteLocation } from "./navigation";

  let discardDialog: ConfirmDialog;
  let startupError = $state("");

  function accessPolicy(location: RouteLocation): string | null {
    const user = $appState.session.user;
    if (location.route.name === "explore" && !$appState.config.public_explore_enabled) return "/";
    if (user?.password_change_required && location.route.name !== "password") {
      return "/account/password";
    }
    const access = routeAccess(location.route);
    if (!user && access !== "public") return "/login";
    if (user && location.route.name === "login") return "/pastes";
    if (access === "admin" && user?.role !== "admin" && user?.role !== "owner") return "/";
    if (access === "owner" && user?.role !== "owner") return "/admin";
    return null;
  }

  async function loadPage(route: Route) {
    const release = holdNavigation();
    try {
      return await loadRouteComponent(route);
    } finally {
      release();
    }
  }

  onMount(() => {
    setConfirmationPrompt((options) => discardDialog.ask(options));
    setDiscardPrompt(() =>
      discardDialog.ask({
        title: "Discard unsaved changes?",
        message: "Your changes will not be saved.",
        confirmLabel: "Discard changes",
        dangerous: true
      })
    );
    let stopNavigation: (() => void) | undefined;
    void loadSession()
      .then(async () => {
        stopNavigation = await startNavigation({
          accessPolicy,
          siteName: () => $appState.config.site_name
        });
      })
      .catch((error) => {
        startupError = error instanceof Error ? error.message : "Unable to start Racebin";
      });
    return () => stopNavigation?.();
  });

  let routeKey = $derived($locationState.path);
  let minimalShell = $derived(!$appState.ready);
</script>

<ConfirmDialog bind:this={discardDialog} />
<Shell minimal={minimalShell}>
  {#if startupError}
    <section class="empty">
      <h1>Unable to load Racebin</h1>
      <p>{startupError}</p>
    </section>
  {:else if !$appState.ready || !$navigationReady}
    <p class="muted">Loading Racebin…</p>
  {:else}
    {#key routeKey}
      {@const route = $locationState.route}
      {#await loadPage(route)}
        <p class="muted">Loading page…</p>
      {:then module}
        {@const Page = module.default}
        <Page {...routeProps(route, $locationState.query)} />
      {:catch error}
        <section class="empty">
          <h1>Unable to load this page</h1>
          <p>{error instanceof Error ? error.message : "The page could not be loaded."}</p>
        </section>
      {/await}
    {/key}
  {/if}
</Shell>
