<script lang="ts">
  import { languageOptions, normalizeLanguage } from "../highlighting";

  let {
    value = $bindable(),
    disabled = false
  }: {
    value: string;
    disabled?: boolean;
  } = $props();

  let input: HTMLInputElement;
  let open = $state(false);
  let query = $state("");
  let active = $state(-1);
  let filtered = $derived(languageOptions.filter(language => {
    const term = query.trim().toLowerCase();
    return !term || [language.id, language.label, ...(language.aliases ?? [])]
      .join(" ").toLowerCase().includes(term);
  }));

  function show(): void {
    if (disabled) return;
    query = "";
    open = true;
    active = -1;
    input.select();
  }

  function filter(): void {
    query = input.value;
    open = true;
    active = -1;
  }

  function choose(language: string): void {
    value = language;
    query = "";
    open = false;
  }

  function blur(): void {
    window.setTimeout(() => {
      open = false;
      const normalized = normalizeLanguage(input.value);
      if (normalized) value = normalized;
      else input.value = value;
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
      input.value = value;
    }
  }
</script>

<div class="language-field">
  <label for="language-input">Language <small>Type to filter languages.</small></label>
  <div class="language-picker">
    <input bind:this={input} id="language-input" name="language" {disabled}
      value={value} autocomplete="off" role="combobox" aria-autocomplete="list"
      aria-expanded={open} aria-controls="language-options-menu" placeholder="Type or choose"
      onfocus={show} oninput={filter} onblur={blur} onkeydown={keydown}/>
    {#if open}
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
