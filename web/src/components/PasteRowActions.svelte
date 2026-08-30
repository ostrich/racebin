<script lang="ts">
  import type { Paste } from "../types";
  import Icon from "./Icon.svelte";
  import Link from "./Link.svelte";

  let {
    paste,
    manage = false,
    oncopy,
    onremove
  }: {
    paste: Paste;
    manage?: boolean;
    oncopy: (paste: Paste) => void;
    onremove: (paste: Paste) => void;
  } = $props();
</script>

<div class="row-actions">
  <button
    class="icon-button"
    type="button"
    title="Copy link"
    aria-label="Copy link"
    onclick={() => oncopy(paste)}><Icon name="link-2" /></button
  >
  {#if manage}
    <Link class="icon-button" title="Edit" aria-label="Edit" href={`/pastes/${paste.id}/edit`}
      ><Icon name="edit-3" /></Link
    >
    <button
      class="icon-button"
      type="button"
      title="Delete"
      aria-label="Delete"
      onclick={() => onremove(paste)}><Icon name="trash-2" /></button
    >
  {/if}
</div>
