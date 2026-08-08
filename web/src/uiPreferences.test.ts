import { get } from "svelte/store";
import { beforeEach, describe, expect, it } from "vitest";
import {
  initializeUiPreferences,
  setColorTheme,
  setFolderSidebarCollapsed,
  setPasteListView,
  uiPreferences
} from "./uiPreferences";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() { return values.size; },
    clear: () => values.clear(),
    getItem: key => values.get(key) ?? null,
    key: index => [...values.keys()][index] ?? null,
    removeItem: key => { values.delete(key); },
    setItem: (key, value) => { values.set(key, value); }
  };
}

describe("UI preferences", () => {
  let storage: Storage;

  beforeEach(() => {
    storage = memoryStorage();
    initializeUiPreferences(storage);
  });

  it("loads the folder sidebar preference before rendering", () => {
    storage.setItem("racebin.folderSidebarCollapsed", "true");
    initializeUiPreferences(storage);

    expect(get(uiPreferences).folderSidebarCollapsed).toBe(true);
  });

  it("loads a valid compact paste-list preference", () => {
    storage.setItem("racebin.pasteListView", "compact");
    initializeUiPreferences(storage);

    expect(get(uiPreferences).pasteListView).toBe("compact");
  });

  it("ignores unknown paste-list preferences", () => {
    storage.setItem("racebin.pasteListView", "dense");
    initializeUiPreferences(storage);

    expect(get(uiPreferences).pasteListView).toBe("normal");
  });

  it("updates memory and storage together", () => {
    setFolderSidebarCollapsed(true, storage);

    expect(get(uiPreferences).folderSidebarCollapsed).toBe(true);
    expect(storage.getItem("racebin.folderSidebarCollapsed")).toBe("true");
  });

  it("updates either preference without resetting the other", () => {
    setFolderSidebarCollapsed(true, storage);
    setPasteListView("compact", storage);

    expect(get(uiPreferences)).toEqual({
      folderSidebarCollapsed: true,
      pasteListView: "compact",
      colorTheme: "auto"
    });
    expect(storage.getItem("racebin.pasteListView")).toBe("compact");

    setFolderSidebarCollapsed(false, storage);
    expect(get(uiPreferences).pasteListView).toBe("compact");
  });

  it("persists explicit themes and returns to the system theme", () => {
    setColorTheme("dark", storage);
    expect(get(uiPreferences).colorTheme).toBe("dark");
    expect(storage.getItem("racebin.colorTheme")).toBe("dark");
    expect(document.documentElement.dataset.colorScheme).toBe("dark");

    setColorTheme("auto", storage);
    expect(get(uiPreferences).colorTheme).toBe("auto");
    expect(storage.getItem("racebin.colorTheme")).toBeNull();
  });
});
