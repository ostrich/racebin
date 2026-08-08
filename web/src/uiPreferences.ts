import { get, writable } from "svelte/store";

const folderSidebarStorageKey = "racebin.folderSidebarCollapsed";
const pasteListViewStorageKey = "racebin.pasteListView";
const colorThemeStorageKey = "racebin.colorTheme";
const systemTheme = typeof matchMedia === "function"
  ? matchMedia("(prefers-color-scheme: dark)")
  : undefined;

export type PasteListView = "normal" | "compact";
export type ColorTheme = "auto" | "dark" | "light";

export type UiPreferences = {
  folderSidebarCollapsed: boolean;
  pasteListView: PasteListView;
  colorTheme: ColorTheme;
};

const defaults: UiPreferences = {
  folderSidebarCollapsed: false,
  pasteListView: "normal",
  colorTheme: "auto"
};

export const uiPreferences = writable<UiPreferences>(defaults);

export function initializeUiPreferences(storage: Storage = localStorage): void {
  let folderSidebarCollapsed = defaults.folderSidebarCollapsed;
  let pasteListView = defaults.pasteListView;
  let colorTheme = defaults.colorTheme;
  try {
    folderSidebarCollapsed = storage.getItem(folderSidebarStorageKey) === "true";
    if (storage.getItem(pasteListViewStorageKey) === "compact") pasteListView = "compact";
    const storedTheme = storage.getItem(colorThemeStorageKey);
    if (storedTheme === "dark" || storedTheme === "light") colorTheme = storedTheme;
  } catch {
    // Storage can be unavailable in privacy-restricted browsing contexts.
  }
  uiPreferences.set({ folderSidebarCollapsed, pasteListView, colorTheme });
  applyColorTheme(colorTheme);
}

function resolvedColorTheme(colorTheme: ColorTheme): "dark" | "light" {
  return colorTheme === "auto"
    ? systemTheme?.matches ? "dark" : "light"
    : colorTheme;
}

function applyColorTheme(colorTheme: ColorTheme): void {
  const root = document.documentElement;
  root.dataset.theme = colorTheme;
  root.dataset.colorScheme = resolvedColorTheme(colorTheme);
  root.style.colorScheme = root.dataset.colorScheme;
}

export function setColorTheme(colorTheme: ColorTheme, storage: Storage = localStorage): void {
  uiPreferences.update(preferences => ({ ...preferences, colorTheme }));
  applyColorTheme(colorTheme);
  try {
    if (colorTheme === "auto") storage.removeItem(colorThemeStorageKey);
    else storage.setItem(colorThemeStorageKey, colorTheme);
  } catch {
    // The in-memory preference remains usable when persistence is unavailable.
  }
}

systemTheme?.addEventListener("change", () => {
  const { colorTheme } = get(uiPreferences);
  if (colorTheme === "auto") applyColorTheme(colorTheme);
});

export function setFolderSidebarCollapsed(
  folderSidebarCollapsed: boolean,
  storage: Storage = localStorage
): void {
  uiPreferences.update(preferences => ({
    ...preferences,
    folderSidebarCollapsed
  }));
  try {
    storage.setItem(folderSidebarStorageKey, String(folderSidebarCollapsed));
  } catch {
    // The in-memory preference remains usable when persistence is unavailable.
  }
}

export function setPasteListView(
  pasteListView: PasteListView,
  storage: Storage = localStorage
): void {
  uiPreferences.update(preferences => ({ ...preferences, pasteListView }));
  try {
    storage.setItem(pasteListViewStorageKey, pasteListView);
  } catch {
    // The in-memory preference remains usable when persistence is unavailable.
  }
}
