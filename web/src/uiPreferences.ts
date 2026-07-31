import { writable } from "svelte/store";

const folderSidebarStorageKey = "racebin.folderSidebarCollapsed";

export type UiPreferences = {
  folderSidebarCollapsed: boolean;
};

const defaults: UiPreferences = {
  folderSidebarCollapsed: false
};

export const uiPreferences = writable<UiPreferences>(defaults);

export function initializeUiPreferences(storage: Storage = localStorage): void {
  let folderSidebarCollapsed = defaults.folderSidebarCollapsed;
  try {
    folderSidebarCollapsed = storage.getItem(folderSidebarStorageKey) === "true";
  } catch {
    // Storage can be unavailable in privacy-restricted browsing contexts.
  }
  uiPreferences.set({ folderSidebarCollapsed });
}

export function setFolderSidebarCollapsed(
  folderSidebarCollapsed: boolean,
  storage: Storage = localStorage
): void {
  uiPreferences.set({ folderSidebarCollapsed });
  try {
    storage.setItem(folderSidebarStorageKey, String(folderSidebarCollapsed));
  } catch {
    // The in-memory preference remains usable when persistence is unavailable.
  }
}
