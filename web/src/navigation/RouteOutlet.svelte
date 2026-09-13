<script lang="ts">
  import { untrack } from "svelte";
  import { holdNavigation } from "./runtime";
  import { loadRouteComponent, routeProps } from "./components";
  import type { HomeRouteVariant } from "./components";
  import RouteReady from "./RouteReady.svelte";
  import type { Route } from "./routes";

  let {
    route,
    query,
    homeVariant
  }: { route: Route; query: URLSearchParams; homeVariant: HomeRouteVariant } = $props();
  const release = holdNavigation();
  const component = untrack(() => loadRouteComponent(route, homeVariant)).catch(
    (error: unknown) => {
      release();
      throw error;
    }
  );
</script>

{#await component}
  <p class="muted">Loading page…</p>
{:then module}
  {@const Page = module.default}
  <Page {...routeProps(route, query)} />
  <RouteReady {release} />
{:catch error}
  <section class="empty">
    <h1>Unable to load this page</h1>
    <p>{error instanceof Error ? error.message : "The page could not be loaded."}</p>
  </section>
{/await}
