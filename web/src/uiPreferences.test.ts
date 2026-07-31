import { get } from "svelte/store";
import { beforeEach, describe, expect, it } from "vitest";
import {
  initializeUiPreferences,
  setFolderSidebarCollapsed,
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

  it("updates memory and storage together", () => {
    setFolderSidebarCollapsed(true, storage);

    expect(get(uiPreferences).folderSidebarCollapsed).toBe(true);
    expect(storage.getItem("racebin.folderSidebarCollapsed")).toBe("true");
  });
});
