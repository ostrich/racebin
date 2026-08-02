<script lang="ts">
  import { formatByteSize } from "../format";
  import { languageOptions } from "../highlighting";
  import { navigate } from "../navigation";
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
    content_kind: "Format", language: "Language", visibility: "Visibility",
    has_attachments: "Attachments", owner_id: "Owner", created_after: "Created after",
    created_before: "Created before", expiration: "Expiration", min_reads: "Minimum reads",
    max_reads: "Maximum reads", min_size_bytes: "Minimum size",
    max_size_bytes: "Maximum size", read_limit: "Read limit"
  };
  const filterKeys = Object.keys(labels);
  const sortChoices = [
    { label: "Newest", sort: "created", direction: "desc", default: true },
    { label: "Oldest", sort: "created", direction: "asc" },
    { label: "Title A–Z", sort: "title", direction: "asc" },
    { label: "Title Z–A", sort: "title", direction: "desc" },
    { label: "Most read", sort: "reads", direction: "desc" },
    { label: "Least read", sort: "reads", direction: "asc" },
    { label: "Largest", sort: "size", direction: "desc" },
    { label: "Smallest", sort: "size", direction: "asc" },
    { label: "Expires soonest", sort: "expires", direction: "asc" },
    { label: "Expires latest", sort: "expires", direction: "desc" }
  ];

  let filtersOpen = $state(false);
  let sortOpen = $state(false);
  let activeFilters = $derived([...params.entries()].filter(
    ([key, value]) => value && key in labels
  ));
  let currentSort = $derived.by(() => {
    const sort = params.get("sort") ?? "created";
    const direction = params.get("direction") === "asc" ? "asc" : "desc";
    return sortChoices.find(choice => choice.sort === sort && choice.direction === direction)
      ?? sortChoices[0]!;
  });

  function dateValue(value: string | null): string {
    if (!value || !Number.isFinite(Number(value))) return "";
    const date = new Date(Number(value) * 1000);
    const part = (number: number) => String(number).padStart(2, "0");
    return `${date.getFullYear()}-${part(date.getMonth() + 1)}-${part(date.getDate())}`;
  }

  function shownValue(key: string, value: string): string {
    if (key === "owner_id") return ownerNames?.get(Number(value)) ?? `User #${value}`;
    if (key === "created_after" || key === "created_before") return dateValue(value);
    if (key === "content_kind") return ({ text: "Text", rich_text: "Rich text" } as Record<string, string>)[value] ?? value;
    if (key === "language") return languageOptions.find(language => language.id === value)?.label ?? value;
    if (key === "visibility") return value.charAt(0).toUpperCase() + value.slice(1);
    if (key === "has_attachments") return value === "true" ? "With attachments" : "Without attachments";
    if (key === "read_limit") return value === "limited" ? "Limited" : "Unlimited";
    if (key === "expiration") return value === "scheduled" ? "Scheduled" : "Never";
    if (key === "min_size_bytes" || key === "max_size_bytes") return formatByteSize(Number(value));
    return value;
  }

  function updatedParams(): URLSearchParams {
    const next = new URLSearchParams(params);
    next.delete("page");
    return next;
  }

  function urlFor(next: URLSearchParams): string {
    return `${location.pathname}${next.size ? `?${next}` : ""}`;
  }

  function without(key: string): string {
    const next = updatedParams();
    next.delete(key);
    return urlFor(next);
  }

  function withoutFilters(): string {
    const next = updatedParams();
    filterKeys.forEach(key => next.delete(key));
    return urlFor(next);
  }

  async function submitSearch(event: SubmitEvent): Promise<void> {
    const data = new FormData(event.currentTarget as HTMLFormElement);
    const next = updatedParams();
    const search = String(data.get("search") ?? "").trim();
    if (search) next.set("search", search);
    else next.delete("search");
    await navigate(urlFor(next));
  }

  async function submitFilters(event: SubmitEvent): Promise<void> {
    const data = new FormData(event.currentTarget as HTMLFormElement);
    const next = updatedParams();
    filterKeys.forEach(key => next.delete(key));
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
    filtersOpen = false;
    await navigate(urlFor(next));
  }

  async function selectSort(choice: typeof sortChoices[number]): Promise<void> {
    const next = updatedParams();
    next.delete("sort");
    next.delete("direction");
    if (!choice.default) {
      next.set("sort", choice.sort);
      next.set("direction", choice.direction);
    }
    sortOpen = false;
    await navigate(urlFor(next));
  }

  function navigateSortMenu(event: KeyboardEvent): void {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    const menu = event.currentTarget as HTMLElement;
    const items = [...menu.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')];
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const target = event.key === "Home" ? 0
      : event.key === "End" ? items.length - 1
      : event.key === "ArrowDown" ? (current + 1) % items.length
      : (current - 1 + items.length) % items.length;
    event.preventDefault();
    items[target]?.focus();
  }

  function closeSort(returnFocus = false): void {
    if (!sortOpen) return;
    sortOpen = false;
    if (returnFocus) requestAnimationFrame(() => document.querySelector<HTMLButtonElement>("#paste-sort-button")?.focus());
  }
</script>

<svelte:window onclick={(event) => {
    if (!(event.target as Element).closest?.(".sort-control")) closeSort();
  }}
  onkeydown={(event) => { if (event.key === "Escape") closeSort(true); }}/>

<div class="paste-filter-form">
  <div class="paste-filter-toolbar">
    <form class="paste-search"
      onsubmit={(event) => { event.preventDefault(); void submitSearch(event); }}>
      <label><span>Search</span><input name="search" value={params.get("search") ?? ""}
        placeholder={mode === "admin" ? "Title, content, ID, owner, file…" : "Title, content, ID, language, file…"}/></label>
      <button class="button primary" type="submit"><Icon name="search"/> Search</button>
    </form>
    <button class="button filter-toggle" type="button" aria-expanded={filtersOpen}
      aria-controls="paste-filter-panel" onclick={() => { filtersOpen = !filtersOpen; }}>
      <Icon name="list-filter"/> Filters
      {#if activeFilters.length}<span class="filter-count">{activeFilters.length}</span>{/if}
    </button>
    <div class="sort-control">
      <button class="button sort-button" id="paste-sort-button" type="button"
        aria-haspopup="menu" aria-expanded={sortOpen} aria-controls="paste-sort-menu"
        onclick={() => {
          sortOpen = !sortOpen;
          if (sortOpen) requestAnimationFrame(() =>
            document.querySelector<HTMLButtonElement>('#paste-sort-menu [aria-checked="true"]')?.focus());
        }}>
        <Icon name="arrow-up-down"/><span>Sort: {currentSort.label}</span><Icon name="chevron-down"/>
      </button>
      {#if sortOpen}
        <div class="sort-menu" id="paste-sort-menu" role="menu" tabindex="-1" onkeydown={navigateSortMenu}>
          {#each sortChoices as choice}
            <button type="button" role="menuitemradio" aria-checked={choice === currentSort}
              onclick={() => void selectSort(choice)}>
              <span>{choice.label}</span>{#if choice === currentSort}<Icon name="check"/>{/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  {#if filtersOpen}
    <form class="filter-panel" id="paste-filter-panel" aria-label="Paste filters"
      onsubmit={(event) => { event.preventDefault(); void submitFilters(event); }}>
      <div class="advanced-filter-grid">
        <label><span>Format</span><select name="content_kind" value={params.get("content_kind") ?? ""}>
          <option value="">Any</option><option value="text">Text</option><option value="rich_text">Rich text</option>
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
      </div>
      <div class="filter-actions">
        <button class="button primary" type="submit">Apply filters</button>
        {#if activeFilters.length}<Link class="button" href={withoutFilters()}>Clear filters</Link>{/if}
      </div>
    </form>
  {/if}

  {#if activeFilters.length}
    <div class="active-filters" aria-label="Active filters">
      {#each activeFilters as [key, value]}
        <Link class="filter-chip" href={without(key)}>
          {labels[key]}: {shownValue(key, value)} <span aria-hidden="true">×</span>
        </Link>
      {/each}
      <Link class="clear-filters" href={withoutFilters()}>Clear filters</Link>
    </div>
  {/if}
</div>
