<script lang="ts">
  import { availableLanguageOptions, normalizeLanguage } from "../highlighting";
  import { appState } from "../state";

  let {
    value = $bindable(),
    disabled = false
  }: {
    value: string;
    disabled?: boolean;
  } = $props();

  let input = $state<HTMLInputElement>();
  let open = $state(false);
  let query = $state("");
  let active = $state(-1);
  let options = $derived(availableLanguageOptions($appState.languages));
  let filtered = $derived(options.filter(language => {
    const term = query.trim().toLowerCase();
    return !term || [language.id, language.label, ...(language.aliases ?? [])]
      .join(" ").toLowerCase().includes(term);
  }));
  function show(): void {
    if (disabled) return;
    query = "";
    open = true;
    active = -1;
    input?.select();
  }

  function filter(event: Event): void {
    query = (event.currentTarget as HTMLInputElement).value;
    open = true;
    active = -1;
  }

  function choose(language: string): void {
    value = language;
    query = "";
    open = false;
  }

  function blur(event: FocusEvent): void {
    const target = event.currentTarget as HTMLInputElement;
    window.setTimeout(() => {
      open = false;
      if (!target.isConnected) return;
      const normalized = normalizeLanguage(target.value);
      if (normalized) value = normalized;
      else target.value = value;
    }, 100);
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      open = true;
      if (!filtered.length) return;
      active = (active + (event.key === "ArrowDown" ? 1 : -1) + filtered.length) % filtered.length;
    } else if (event.key === "Enter" && open && active >= 0) {
      event.preventDefault();
      const option = filtered[active];
      if (option) choose(option.id);
    } else if (event.key === "Escape") {
      open = false;
      (event.currentTarget as HTMLInputElement).value = value;
    }
  }
</script>

<div class="language-field">
  <label for="language-input">Language <small>Type to filter languages.</small></label>
  <div class="language-picker">
    {#if disabled}
      <input id="language-input" name="language" disabled value="Not applicable"
        autocomplete="off" role="combobox" aria-expanded="false"
        aria-controls="language-options-menu" placeholder="Type or choose"/>
    {:else}
      <input bind:this={input} id="language-input" name="language" value={value}
        autocomplete="off" role="combobox" aria-autocomplete="list"
        aria-expanded={open} aria-controls="language-options-menu" placeholder="Type or choose"
        onfocus={show} oninput={filter} onblur={blur} onkeydown={keydown}/>
    {/if}
    {#if open && !disabled}
      <div id="language-options-menu" class="language-options" role="listbox" tabindex="-1"
        onmousedown={(event) => event.preventDefault()}>
        {#each filtered as language, index (language.id)}
          <button type="button" role="option" class:active={index === active}
            class:selected={language.id === value} aria-selected={language.id === value}
            onclick={() => choose(language.id)}>
            {language.label}<small>{language.id}</small>
          </button>
        {:else}<p class="muted language-empty">No matching languages.</p>{/each}
      </div>
    {/if}
  </div>
</div>
