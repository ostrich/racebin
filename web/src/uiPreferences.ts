import { writable } from "svelte/store";

const folderSidebarStorageKey = "racebin.folderSidebarCollapsed";
const pasteListViewStorageKey = "racebin.pasteListView";

export type PasteListView = "normal" | "compact";

export type UiPreferences = {
  folderSidebarCollapsed: boolean;
  pasteListView: PasteListView;
};

const defaults: UiPreferences = {
  folderSidebarCollapsed: false,
  pasteListView: "normal"
};

export const uiPreferences = writable<UiPreferences>(defaults);

export function initializeUiPreferences(storage: Storage = localStorage): void {
  let folderSidebarCollapsed = defaults.folderSidebarCollapsed;
  let pasteListView = defaults.pasteListView;
  try {
    folderSidebarCollapsed = storage.getItem(folderSidebarStorageKey) === "true";
    if (storage.getItem(pasteListViewStorageKey) === "compact") pasteListView = "compact";
  } catch {
    // Storage can be unavailable in privacy-restricted browsing contexts.
  }
  uiPreferences.set({ folderSidebarCollapsed, pasteListView });
}

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
