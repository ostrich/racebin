<script lang="ts">
  import { formatByteSize } from "../format";
  import { languageOptions } from "../highlighting";
  import { navigate } from "../router";
  import Icon from "./Icon.svelte";
  import Link from "./Link.svelte";

  let {
    params,
    mode,
    ownerNames
  }: {
    params: URLSearchParams;
    mode: "mine" | "explore" | "admin";
    ownerNames?: Map<number, string>;
  } = $props();

  const labels: Record<string, string> = {
    search: "Search", content_kind: "Format", language: "Language", visibility: "Visibility",
    has_attachments: "Attachments", owner_id: "Owner", created_after: "Created after",
    created_before: "Created before", expiration: "Expiration", min_reads: "Minimum reads",
    max_reads: "Maximum reads", min_size_bytes: "Minimum size",
    max_size_bytes: "Maximum size", read_limit: "Read limit", sort: "Sort",
    direction: "Direction"
  };
  const advancedKeys = [
    "language", "owner_id", "created_after", "created_before", "expiration",
    "min_reads", "max_reads", "min_size_bytes", "max_size_bytes", "read_limit",
    "sort", "direction"
  ];
  let advanced = $derived(advancedKeys.some(key => params.has(key)));
  let chips = $derived([...params.entries()].filter(
    ([key, value]) => value && key in labels && key !== "page_size"
  ));

  function dateValue(value: string | null): string {
    if (!value || !Number.isFinite(Number(value))) return "";
    const date = new Date(Number(value) * 1000);
    const part = (number: number) => String(number).padStart(2, "0");
    return `${date.getFullYear()}-${part(date.getMonth() + 1)}-${part(date.getDate())}`;
  }

  function shownValue(key: string, value: string): string {
    if (key === "owner_id") return ownerNames?.get(Number(value)) ?? `User #${value}`;
    if (key === "created_after" || key === "created_before") return dateValue(value);
    if (key === "content_kind") return ({ text: "Text", rich_text: "Rich text", redirect: "Redirect" } as Record<string, string>)[value] ?? value;
    if (key === "language") return languageOptions.find(language => language.id === value)?.label ?? value;
    if (key === "visibility") return value.charAt(0).toUpperCase() + value.slice(1);
    if (key === "has_attachments") return value === "true" ? "With attachments" : "Without attachments";
    if (key === "read_limit") return value === "limited" ? "Limited" : "Unlimited";
    if (key === "expiration") return value === "scheduled" ? "Scheduled" : "Never";
    if (key === "sort") return ({ created: "Created", title: "Title", reads: "Reads", expires: "Expiration", size: "Size" } as Record<string, string>)[value] ?? value;
    if (key === "direction") return value === "asc" ? "Ascending" : "Descending";
    if (key === "min_size_bytes" || key === "max_size_bytes") return formatByteSize(Number(value));
    return value;
  }

  function without(key: string): string {
    const next = new URLSearchParams(params);
    next.delete(key);
    next.delete("page");
    return `${location.pathname}${next.size ? `?${next}` : ""}`;
  }

  async function submit(event: SubmitEvent): Promise<void> {
    const data = new FormData(event.currentTarget as HTMLFormElement);
    const next = new URLSearchParams();
    data.forEach((value, key) => {
      if (!value || value instanceof File) return;
      if (key === "created_after" || key === "created_before") {
        const suffix = key === "created_before" ? "T23:59:59" : "T00:00:00";
        next.set(key, String(Math.floor(new Date(`${value}${suffix}`).getTime() / 1000)));
      } else if (key === "min_size_kib" || key === "max_size_kib") {
        next.set(key.replace("_kib", "_bytes"), String(Math.round(Number(value) * 1024)));
      } else {
        next.set(key, String(value));
      }
    });
    await navigate(`${location.pathname}${next.size ? `?${next}` : ""}`);
  }
</script>

<form class="paste-filter-form" onsubmit={(event) => { event.preventDefault(); void submit(event); }}>
  <div class="paste-filter-primary">
    <label><span>Search</span><input name="search" value={params.get("search") ?? ""}
      placeholder={mode === "admin" ? "Title, content, ID, owner, file…" : "Title, content, ID, language, file…"}/></label>
    <label><span>Format</span><select name="content_kind" value={params.get("content_kind") ?? ""}>
      <option value="">Any</option><option value="text">Text</option>
      <option value="rich_text">Rich text</option><option value="redirect">Redirect</option>
    </select></label>
    {#if mode !== "explore"}
      <label><span>Visibility</span><select name="visibility" value={params.get("visibility") ?? ""}>
        <option value="">Any</option><option value="public">Public</option>
        <option value="unlisted">Unlisted</option><option value="private">Private</option>
      </select></label>
    {/if}
    <label><span>Attachments</span><select name="has_attachments" value={params.get("has_attachments") ?? ""}>
      <option value="">Any</option><option value="true">With attachments</option>
      <option value="false">Without attachments</option>
    </select></label>
    <button class="button primary" type="submit"><Icon name="search"/> Apply</button>
  </div>
  <details class="advanced-filters" open={advanced}>
    <summary>More filters</summary>
    <div class="advanced-filter-grid">
      <label><span>Language</span><select name="language" value={params.get("language") ?? ""}>
        <option value="">Any</option>
        {#each languageOptions.filter(language => language.id !== "auto") as language}
          <option value={language.id}>{language.label}</option>
        {/each}
      </select></label>
      {#if mode === "admin"}
        <label><span>Owner ID</span><input type="number" min="1" name="owner_id" value={params.get("owner_id") ?? ""}/></label>
      {/if}
      <label><span>Created after</span><input type="date" name="created_after" value={dateValue(params.get("created_after"))}/></label>
      <label><span>Created before</span><input type="date" name="created_before" value={dateValue(params.get("created_before"))}/></label>
      <label><span>Expiration</span><select name="expiration" value={params.get("expiration") ?? ""}>
        <option value="">Any</option><option value="never">Never</option><option value="scheduled">Scheduled</option>
      </select></label>
      <label><span>Minimum reads</span><input type="number" min="0" name="min_reads" value={params.get("min_reads") ?? ""}/></label>
      <label><span>Maximum reads</span><input type="number" min="0" name="max_reads" value={params.get("max_reads") ?? ""}/></label>
      <label><span>Minimum size (KiB)</span><input type="number" min="0" step="0.1" name="min_size_kib"
        value={params.get("min_size_bytes") ? Number(params.get("min_size_bytes")) / 1024 : ""}/></label>
      <label><span>Maximum size (KiB)</span><input type="number" min="0" step="0.1" name="max_size_kib"
        value={params.get("max_size_bytes") ? Number(params.get("max_size_bytes")) / 1024 : ""}/></label>
      <label><span>Read limit</span><select name="read_limit" value={params.get("read_limit") ?? ""}>
        <option value="">Any</option><option value="unlimited">Unlimited</option><option value="limited">Limited</option>
      </select></label>
      <label><span>Sort by</span><select name="sort" value={params.get("sort") ?? ""}>
        <option value="">Any</option><option value="created">Created</option><option value="title">Title</option>
        <option value="reads">Reads</option><option value="expires">Expiration</option><option value="size">Size</option>
      </select></label>
      <label><span>Direction</span><select name="direction" value={params.get("direction") ?? ""}>
        <option value="">Any</option><option value="desc">Descending</option><option value="asc">Ascending</option>
      </select></label>
    </div>
  </details>
  {#if chips.length}
    <div class="active-filters">
      {#each chips as [key, value]}
        <Link class="filter-chip" href={without(key)}>
          {labels[key]}: {shownValue(key, value)} <span aria-hidden="true">×</span>
        </Link>
      {/each}
      <Link class="clear-filters" href={location.pathname}>Clear all</Link>
    </div>
  {/if}
</form>
