<script lang="ts">
  import { onMount } from "svelte";
  import { listAuditEvents, type AuditEvent } from "../api";
  import AdminNav from "../components/AdminNav.svelte";
  import { formatDate } from "../format";
  import { holdNavigation } from "../navigation";

  let events = $state<AuditEvent[]>([]);
  let error = $state("");
  let search = $state("");
  const initialLoadReady = holdNavigation();
  let filtered = $derived(events.filter(event =>
    `${event.actor_username} ${event.action} ${event.target_type} ${event.target_id ?? ""} ${event.target_label ?? ""}`
      .toLowerCase().includes(search.toLowerCase())
  ));

  onMount(() => {
    void listAuditEvents()
      .then(value => { events = value; })
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load audit log"; })
      .finally(initialLoadReady);
  });
</script>

<section class="page-layout">
  <div class="page-heading">
    <div><p class="eyebrow">Owner</p><h1>Audit log</h1></div>
  </div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content">
      <div class="list-filter-bar"><label class="field list-filter-search"><span>Search</span><input type="search" placeholder="Actor, action, or target" bind:value={search}></label></div>
      {#if error}
        <section class="empty"><p>{error}</p></section>
      {:else}
        <div class="panel data-list">
          {#each filtered as event (event.id)}
            <article class="data-row">
              <div>
                <strong>{event.action.replaceAll(".", " ")}</strong>
                <small>{event.actor_username} · {formatDate(event.created_at)}{event.target_label ? ` · ${event.target_label}` : event.target_id ? ` · ${event.target_type} ${event.target_id}` : ""}</small>
              </div>
            </article>
          {:else}
            <div class="empty"><p>No audit events match.</p></div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</section>
