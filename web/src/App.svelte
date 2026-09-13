<script lang="ts">
  import { onMount } from "svelte";
  import ConfirmDialog from "./components/ConfirmDialog.svelte";
  import Shell from "./components/Shell.svelte";
  import { setConfirmationPrompt } from "./app/confirmations";
  import { bootstrapApplication, replaceSession } from "./app/session";
  import { showNotice } from "./app/notices";
  import { setSessionInvalidHandler } from "./api";
  import { appState } from "./app/state";
  import RouteOutlet from "./navigation/RouteOutlet.svelte";
  import {
    locationState,
    navigate,
    navigationReady,
    routeAccess,
    setDiscardPrompt,
    startNavigation
  } from "./navigation";
  import type { RouteLocation } from "./navigation";

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
    setSessionInvalidHandler(() => {
      replaceSession({ authenticated: false, permissions: [] });
      showNotice("Your session has ended. Log in again to continue.", "error");
      void navigate("/login", { replace: true, discardConfirmed: true });
    });
    let stopNavigation: (() => void) | undefined;
    void bootstrapApplication()
      .then(async () => {
        stopNavigation = await startNavigation({
          accessPolicy,
          siteName: () => $appState.config.site_name
        });
      })
      .catch((error) => {
        startupError = error instanceof Error ? error.message : "Unable to start Racebin";
      });
    return () => {
      setSessionInvalidHandler();
      stopNavigation?.();
    };
  });

  let routeKey = $derived(
    `${$locationState.route.name}:${"pasteId" in $locationState.route ? $locationState.route.pasteId : ""}:${"userId" in $locationState.route ? $locationState.route.userId : ""}:${"token" in $locationState.route ? $locationState.route.token : ""}`
  );
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
      <RouteOutlet route={$locationState.route} query={$locationState.query} />
    {/key}
  {/if}
</Shell>
