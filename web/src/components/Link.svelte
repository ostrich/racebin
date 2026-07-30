<script lang="ts">
  import { navigate } from "../router";

  let {
    href,
    class: className = "",
    children,
    ...attributes
  }: {
    href: string;
    class?: string;
    children: import("svelte").Snippet;
    [key: string]: unknown;
  } = $props();

  function activate(event: MouseEvent): void {
    if (
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) return;
    event.preventDefault();
    void navigate(href);
  }
</script>

<a {href} class={className} onclick={activate} {...attributes}>{@render children()}</a>
