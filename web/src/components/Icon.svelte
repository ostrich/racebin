<script lang="ts">
  import { icons, type IconName } from "./icons";

  let { name }: { name: IconName } = $props();
  let Glyph = $derived(icons[name]);
</script>

<Glyph aria-hidden="true"/>
