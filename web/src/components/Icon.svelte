<script lang="ts">
  let { name }: { name: string } = $props();
</script>

<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
  stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if name === "copy"}
    <rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
  {:else if name === "edit-3"}
    <path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L8 18l-4 1 1-4Z"/>
  {:else if name === "file-text"}
    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z"/><path d="M14 2v6h6M8 13h8M8 17h8"/>
  {:else if name === "key-round"}
    <circle cx="8" cy="15" r="5"/><path d="m12 11 9-9M18 5l3 3M15 8l3 3"/>
  {:else if name === "log-in"}
    <path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M15 12H3"/>
  {:else if name === "log-out"}
    <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/>
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14"/>
  {:else if name === "search"}
    <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
  {:else if name === "trash-2"}
    <path d="M3 6h18M8 6V4h8v2M19 6l-1 15H6L5 6M10 11v6M14 11v6"/>
  {:else if name === "user-round"}
    <circle cx="12" cy="8" r="5"/><path d="M4 21a8 8 0 0 1 16 0"/>
  {/if}
</svg>
