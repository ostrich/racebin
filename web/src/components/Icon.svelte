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
  {:else if name === "more-horizontal"}
    <circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>
  {:else if name === "panel-left-close"}
    <rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18M16 15l-3-3 3-3"/>
  {:else if name === "panel-left-open"}
    <rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18M14 9l3 3-3 3"/>
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14"/>
  {:else if name === "search"}
    <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
  {:else if name === "list-filter"}
    <path d="M3 6h18M7 12h10M10 18h4"/>
  {:else if name === "arrow-up-down"}
    <path d="m21 16-4 4-4-4M17 20V4M3 8l4-4 4 4M7 4v16"/>
  {:else if name === "chevron-down"}
    <path d="m6 9 6 6 6-6"/>
  {:else if name === "check"}
    <path d="m20 6-11 11-5-5"/>
  {:else if name === "trash-2"}
    <path d="M3 6h18M8 6V4h8v2M19 6l-1 15H6L5 6M10 11v6M14 11v6"/>
  {:else if name === "user-round"}
    <circle cx="12" cy="8" r="5"/><path d="M4 21a8 8 0 0 1 16 0"/>
  {:else if name === "link"}
    <path d="M10 13a5 5 0 0 0 7.1.1l2-2a5 5 0 0 0-7.1-7.1l-1.1 1.1"/><path d="M14 11a5 5 0 0 0-7.1-.1l-2 2a5 5 0 0 0 7.1 7.1l1.1-1.1"/>
  {:else if name === "list"}
    <path d="M8 6h13M8 12h13M8 18h13"/><path d="M3 6h.01M3 12h.01M3 18h.01"/>
  {:else if name === "list-ordered"}
    <path d="M10 6h11M10 12h11M10 18h11"/><path d="M4 6h1V3M4 10h2l-2 3h2M4 17h2v4H4"/>
  {:else if name === "quote"}
    <path d="M3 21c3 0 7-1 7-8V5c0-1.25-.75-2-2-2H4c-1.25 0-2 .75-2 2v6c0 1.25.75 2 2 2h2c0 3-1 4-3 5M14 21c3 0 7-1 7-8V5c0-1.25-.75-2-2-2h-4c-1.25 0-2 .75-2 2v6c0 1.25.75 2 2 2h2c0 3-1 4-3 5"/>
  {:else if name === "code"}
    <path d="m8 9-3 3 3 3M16 9l3 3-3 3M14 5l-4 14"/>
  {:else if name === "square-code"}
    <rect width="18" height="18" x="3" y="3" rx="2"/><path d="m10 9-3 3 3 3M14 9l3 3-3 3"/>
  {:else if name === "minus"}
    <path d="M5 12h14"/>
  {:else if name === "align-left"}
    <path d="M15 6H3M21 12H3M15 18H3"/>
  {:else if name === "align-center"}
    <path d="M17 6H7M21 12H3M17 18H7"/>
  {:else if name === "align-right"}
    <path d="M21 6H9M21 12H3M21 18H9"/>
  {:else if name === "eraser"}
    <path d="m7 21-4-4a2 2 0 0 1 0-3L14 3a2 2 0 0 1 3 0l4 4a2 2 0 0 1 0 3L10 21M5 12l7 7M7 21h14"/>
  {:else if name === "undo-2"}
    <path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>
  {:else if name === "redo-2"}
    <path d="m15 14 5-5-5-5"/><path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13"/>
  {/if}
</svg>
