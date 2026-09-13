<script lang="ts">
  import { appState } from "../app/state";
  import HomePage from "./HomePage.svelte";
  import LoginPage from "./LoginPage.svelte";
  import PasteFormPage from "./PasteFormPage.svelte";
</script>

{#if $appState.session.user}
  <PasteFormPage />
{:else if $appState.config.plain_home_enabled}
  <LoginPage />
{:else}
  <HomePage />
{/if}
