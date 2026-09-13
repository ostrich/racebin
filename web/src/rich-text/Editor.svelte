<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Editor } from "@tiptap/core";
  import { Markdown } from "@tiptap/markdown";
  import { TableKit } from "@tiptap/extension-table";
  import TaskList from "@tiptap/extension-task-list";
  import TaskItem from "@tiptap/extension-task-item";
  import StarterKit from "@tiptap/starter-kit";
  import { showNotice } from "../app/notices";
  import { confirmAction } from "../app/confirmations";
  import Icon from "../components/Icon.svelte";
  import TextInputDialog from "../components/TextInputDialog.svelte";
  import { RichTextPasteNormalization } from "./pasteNormalization";
  import type { IconName } from "../components/icons";

  let { markdown = $bindable(), onchange }: { markdown: string; onchange?: () => void } = $props();
  let element: HTMLDivElement;
  let editor: Editor;
  let linkDialog: TextInputDialog;
  let tableTool = $state<HTMLDivElement>();
  let tablePickerOpen = $state(false);
  let tableRows = $state(1);
  let tableColumns = $state(1);
  let tablePickerTop = $state(0);
  let tablePickerLeft = $state(0);
  let tablePickerPositioned = $state(false);
  let insideTable = $state(false);
  let ready = $state(false);
  let alive = false;
  let activeCommands = $state(new Set<string>());
  const tablePickerSize = 8;
  const tableIncompatibleCommands = new Set([
    "heading-1",
    "heading-2",
    "heading-3",
    "bullet-list",
    "ordered-list",
    "task-list",
    "table",
    "blockquote",
    "code-block",
    "horizontal-rule"
  ]);
  const commandDisabled = (command: string) =>
    insideTable && tableIncompatibleCommands.has(command);
  const commandTitle = (command: string, label: string) =>
    commandDisabled(command) ? `${label} is not supported inside Markdown table cells` : label;
  const toggleCommands = new Set([
    "paragraph",
    "heading-1",
    "heading-2",
    "heading-3",
    "bold",
    "italic",
    "strike",
    "link",
    "bullet-list",
    "ordered-list",
    "task-list",
    "blockquote",
    "code",
    "code-block"
  ]);
  const tableSizeLabel = (rows: number, columns: number) =>
    `${rows} ${rows === 1 ? "row" : "rows"} by ${columns} ${columns === 1 ? "column" : "columns"}`;

  function safeLink(href: string): boolean {
    try {
      return ["http:", "https:", "mailto:"].includes(new URL(href, location.origin).protocol);
    } catch {
      return false;
    }
  }
  const commands: Array<{
    command: string;
    label: string;
    icon?: IconName;
    symbol?: string;
    symbolClass?: string;
  }> = [
    { command: "paragraph", label: "Paragraph", symbol: "¶", symbolClass: "paragraph" },
    { command: "heading-1", label: "Heading 1", symbol: "H1" },
    { command: "heading-2", label: "Heading 2", symbol: "H2" },
    { command: "heading-3", label: "Heading 3", symbol: "H3" },
    { command: "bold", label: "Bold", symbol: "B", symbolClass: "bold" },
    { command: "italic", label: "Italic", symbol: "I", symbolClass: "italic" },
    { command: "strike", label: "Strikethrough", symbol: "S", symbolClass: "strike" },
    { command: "link", label: "Link", icon: "link" },
    { command: "bullet-list", label: "Bulleted list", icon: "list" },
    { command: "ordered-list", label: "Numbered list", icon: "list-ordered" },
    { command: "task-list", label: "Task list", icon: "list-checks" },
    { command: "table", label: "Insert table", icon: "table-2" },
    { command: "blockquote", label: "Block quote", icon: "quote" },
    { command: "code", label: "Inline code", icon: "code" },
    { command: "code-block", label: "Code block", icon: "square-code" },
    { command: "horizontal-rule", label: "Horizontal rule", icon: "minus" },
    { command: "clear-formatting", label: "Clear all formatting", icon: "eraser" },
    { command: "undo", label: "Undo", icon: "undo-2" },
    { command: "redo", label: "Redo", icon: "redo-2" }
  ];
  async function run(command: string): Promise<void> {
    if (commandDisabled(command)) return;
    const chain = editor.chain().focus();
    switch (command) {
      case "paragraph":
        chain.setParagraph().run();
        break;
      case "heading-1":
        chain.toggleHeading({ level: 1 }).run();
        break;
      case "heading-2":
        chain.toggleHeading({ level: 2 }).run();
        break;
      case "heading-3":
        chain.toggleHeading({ level: 3 }).run();
        break;
      case "bold":
        chain.toggleBold().run();
        break;
      case "italic":
        chain.toggleItalic().run();
        break;
      case "strike":
        chain.toggleStrike().run();
        break;
      case "code":
        chain.toggleCode().run();
        break;
      case "bullet-list":
        chain.toggleBulletList().run();
        break;
      case "ordered-list":
        chain.toggleOrderedList().run();
        break;
      case "task-list":
        chain.toggleTaskList().run();
        break;
      case "table":
        void openTablePicker();
        break;
      case "blockquote":
        chain.toggleBlockquote().run();
        break;
      case "code-block":
        chain.toggleCodeBlock().run();
        break;
      case "horizontal-rule":
        chain.setHorizontalRule().run();
        break;
      case "undo":
        chain.undo().run();
        break;
      case "redo":
        chain.redo().run();
        break;
      case "clear-formatting":
        if (
          await confirmAction({
            title: "Clear all formatting?",
            message: "All rich-text formatting will be removed from this paste.",
            confirmLabel: "Clear formatting",
            dangerous: true
          })
        )
          chain.selectAll().unsetAllMarks().clearNodes().run();
        break;
      case "link": {
        const current = editor.getAttributes("link").href as string | undefined;
        const href = await linkDialog.ask({
          title: current ? "Edit link" : "Add link",
          label: "Link URL",
          value: current ?? "https://",
          submitLabel: "Apply",
          allowEmpty: true
        });
        if (href === null) break;
        if (!href.trim()) chain.unsetLink().run();
        else if (!safeLink(href.trim()))
          showNotice("Links support only HTTP, HTTPS, email, and relative URLs.", "error");
        else chain.extendMarkRange("link").setLink({ href: href.trim() }).run();
      }
    }
  }
  async function openTablePicker(): Promise<void> {
    tablePickerOpen = !tablePickerOpen;
    tablePickerPositioned = false;
    tableRows = 1;
    tableColumns = 1;
    if (tablePickerOpen) {
      await tick();
      const trigger = tableTool?.querySelector<HTMLButtonElement>(".table-picker-trigger");
      const picker = tableTool?.querySelector<HTMLElement>(".table-picker");
      if (trigger && picker) {
        const triggerBox = trigger.getBoundingClientRect();
        const toolbarBox = trigger.closest(".rich-text-toolbar")!.getBoundingClientRect();
        const pickerBox = picker.getBoundingClientRect();
        tablePickerLeft = Math.max(
          8,
          Math.min(triggerBox.left, window.innerWidth - pickerBox.width - 8)
        );
        const below = toolbarBox.bottom + 6;
        tablePickerTop =
          below + pickerBox.height <= window.innerHeight - 8
            ? below
            : Math.max(8, toolbarBox.top - pickerBox.height - 6);
        tablePickerPositioned = true;
      }
      requestAnimationFrame(() => {
        tableTool?.querySelector<HTMLButtonElement>('[data-table-cell="1-1"]')?.focus();
      });
    }
  }
  function insertTable(rows: number, columns: number): void {
    editor.chain().focus().insertTable({ rows, cols: columns, withHeaderRow: true }).run();
    tablePickerOpen = false;
    insideTable = true;
  }
  function tablePickerKeydown(event: KeyboardEvent, row: number, column: number): void {
    if (event.key === "Escape") {
      event.preventDefault();
      tablePickerOpen = false;
      tableTool?.querySelector<HTMLButtonElement>(".table-picker-trigger")?.focus();
      return;
    }
    const movement: Record<string, [number, number]> = {
      ArrowUp: [-1, 0],
      ArrowDown: [1, 0],
      ArrowLeft: [0, -1],
      ArrowRight: [0, 1]
    };
    const delta = movement[event.key];
    if (!delta) return;
    event.preventDefault();
    const nextRow = Math.min(tablePickerSize, Math.max(1, row + delta[0]));
    const nextColumn = Math.min(tablePickerSize, Math.max(1, column + delta[1]));
    tableRows = nextRow;
    tableColumns = nextColumn;
    tableTool
      ?.querySelector<HTMLButtonElement>(`[data-table-cell="${nextRow}-${nextColumn}"]`)
      ?.focus();
  }
  function tableCommand(
    command:
      | "addRowBefore"
      | "addRowAfter"
      | "deleteRow"
      | "addColumnBefore"
      | "addColumnAfter"
      | "deleteColumn"
      | "deleteTable"
  ): void {
    editor.chain().focus()[command]().run();
    insideTable = editor.isActive("table");
  }
  function updateCommandState(updated: Editor): void {
    insideTable = updated.isActive("table");
    activeCommands = new Set(
      [
        updated.isActive("paragraph") && "paragraph",
        updated.isActive("heading", { level: 1 }) && "heading-1",
        updated.isActive("heading", { level: 2 }) && "heading-2",
        updated.isActive("heading", { level: 3 }) && "heading-3",
        updated.isActive("bold") && "bold",
        updated.isActive("italic") && "italic",
        updated.isActive("strike") && "strike",
        updated.isActive("link") && "link",
        updated.isActive("bulletList") && "bullet-list",
        updated.isActive("orderedList") && "ordered-list",
        updated.isActive("taskList") && "task-list",
        updated.isActive("blockquote") && "blockquote",
        updated.isActive("code") && "code",
        updated.isActive("codeBlock") && "code-block"
      ].filter((command): command is string => Boolean(command))
    );
  }
  function scheduleCommandState(updated: Editor): void {
    queueMicrotask(() => {
      if (alive && !updated.isDestroyed) updateCommandState(updated);
    });
  }
  onMount(() => {
    alive = true;
    const closePicker = (event: PointerEvent) => {
      if (tablePickerOpen && !tableTool?.contains(event.target as Node)) tablePickerOpen = false;
    };
    document.addEventListener("pointerdown", closePicker);
    editor = new Editor({
      element,
      extensions: [
        StarterKit.configure({
          heading: { levels: [1, 2, 3, 4, 5, 6] },
          dropcursor: false,
          gapcursor: false,
          underline: false,
          link: {
            openOnClick: false,
            autolink: true,
            protocols: ["http", "https", "mailto"],
            isAllowedUri: safeLink,
            HTMLAttributes: { rel: "noopener noreferrer nofollow", target: "_blank" }
          }
        }),
        RichTextPasteNormalization,
        TableKit,
        TaskList,
        TaskItem.configure({ nested: true }),
        Markdown.configure({ markedOptions: { gfm: true } })
      ],
      content: markdown,
      contentType: "markdown",
      editorProps: {
        attributes: { class: "rich-text-content", "aria-label": "Rich-text paste content" }
      },
      onUpdate: ({ editor: updated }) => {
        markdown = updated.getMarkdown();
        onchange?.();
      },
      onSelectionUpdate: ({ editor: updated }) => {
        scheduleCommandState(updated);
      },
      onTransaction: ({ editor: updated }) => {
        scheduleCommandState(updated);
      }
    });
    updateCommandState(editor);
    ready = true;
    return () => {
      alive = false;
      document.removeEventListener("pointerdown", closePicker);
      editor.destroy();
    };
  });
</script>

<TextInputDialog bind:this={linkDialog} />

<div class="rich-text-toolbar" role="toolbar" aria-label="Rich-text formatting">
  {#each commands as item}
    {#if item.command === "table"}
      <div class="table-tool" bind:this={tableTool}>
        <button
          type="button"
          class="table-picker-trigger"
          title={commandTitle(item.command, item.label)}
          aria-label={item.label}
          disabled={commandDisabled(item.command)}
          aria-haspopup="grid"
          aria-expanded={tablePickerOpen}
          onclick={() => run(item.command)}
        >
          <Icon name="table-2" />
        </button>
        {#if tablePickerOpen}
          <div
            class:positioned={tablePickerPositioned}
            class="table-picker"
            role="dialog"
            aria-label="Choose table size"
            style={`top:${tablePickerTop}px;left:${tablePickerLeft}px`}
          >
            <div
              class="table-picker-grid"
              role="grid"
              aria-label={`${tableRows} rows by ${tableColumns} columns`}
            >
              {#each Array(tablePickerSize) as _, row}
                <div role="row">
                  {#each Array(tablePickerSize) as _, column}
                    <button
                      type="button"
                      role="gridcell"
                      tabindex={row + 1 === tableRows && column + 1 === tableColumns ? 0 : -1}
                      data-table-cell={`${row + 1}-${column + 1}`}
                      class:selected={row < tableRows && column < tableColumns}
                      aria-label={tableSizeLabel(row + 1, column + 1)}
                      onmouseenter={() => {
                        tableRows = row + 1;
                        tableColumns = column + 1;
                      }}
                      onfocus={() => {
                        tableRows = row + 1;
                        tableColumns = column + 1;
                      }}
                      onkeydown={(event) => tablePickerKeydown(event, row + 1, column + 1)}
                      onclick={() => insertTable(row + 1, column + 1)}
                    ></button>
                  {/each}
                </div>
              {/each}
            </div>
            <output aria-live="polite">{tableRows} × {tableColumns} table</output>
          </div>
        {/if}
      </div>
    {:else}
      <button
        type="button"
        title={commandTitle(item.command, item.label)}
        aria-label={item.label}
        class:active={activeCommands.has(item.command)}
        aria-pressed={toggleCommands.has(item.command)
          ? activeCommands.has(item.command)
          : undefined}
        disabled={commandDisabled(item.command)}
        onclick={() => run(item.command)}
      >
        {#if item.icon}<Icon name={item.icon} />{:else}<span
            class:paragraph={item.symbolClass === "paragraph"}
            class:bold={item.symbolClass === "bold"}
            class:italic={item.symbolClass === "italic"}
            class:strike={item.symbolClass === "strike"}
            aria-hidden="true">{item.symbol}</span
          >{/if}
      </button>
    {/if}
  {/each}
  {#if insideTable}
    <div class="table-edit-controls" role="group" aria-label="Edit table">
      <button
        type="button"
        title="Add row above"
        aria-label="Add row above"
        onclick={() => tableCommand("addRowBefore")}>+R↑</button
      >
      <button
        type="button"
        title="Add row below"
        aria-label="Add row below"
        onclick={() => tableCommand("addRowAfter")}>+R↓</button
      >
      <button
        type="button"
        title="Delete row"
        aria-label="Delete row"
        onclick={() => tableCommand("deleteRow")}>−R</button
      >
      <button
        type="button"
        title="Add column left"
        aria-label="Add column left"
        onclick={() => tableCommand("addColumnBefore")}>+C←</button
      >
      <button
        type="button"
        title="Add column right"
        aria-label="Add column right"
        onclick={() => tableCommand("addColumnAfter")}>+C→</button
      >
      <button
        type="button"
        title="Delete column"
        aria-label="Delete column"
        onclick={() => tableCommand("deleteColumn")}>−C</button
      >
      <button
        type="button"
        title="Delete table"
        aria-label="Delete table"
        onclick={() => tableCommand("deleteTable")}><Icon name="trash-2" /></button
      >
    </div>
  {/if}
</div>
<div bind:this={element} class="rich-text-editor" data-editor-ready={ready}></div>
