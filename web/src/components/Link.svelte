<script lang="ts">
  import { navigate } from "../navigation";
  import { prefetchRoute } from "../navigation/components";

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
    const anchor = event.currentTarget as HTMLAnchorElement;
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey ||
      Boolean(anchor.target && anchor.target !== "_self") ||
      anchor.hasAttribute("download") ||
      new URL(anchor.href, location.href).origin !== location.origin
    )
      return;
    event.preventDefault();
    void navigate(href);
  }
</script>

<a
  {href}
  class={className}
  onclick={activate}
  onpointerenter={() => prefetchRoute(href)}
  onfocus={() => prefetchRoute(href)}
  {...attributes}>{@render children()}</a
>
