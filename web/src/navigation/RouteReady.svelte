<script lang="ts">
  import { onMount, tick } from "svelte";

  let { release }: { release: () => void } = $props();

  onMount(() => {
    void tick().then(release);
    return release;
  });
</script>
