import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  clearUnsavedChangesGuard,
  confirmDiscardChanges,
  guardUnsavedChanges,
  hasUnsavedChanges,
  setDiscardPrompt,
  startUnloadGuard
} from "./guards";

describe("navigation guards", () => {
  beforeEach(() => {
    clearUnsavedChangesGuard();
    setDiscardPrompt(async () => true);
  });

  it("only prompts when the active form reports unsaved changes", async () => {
    const prompt = vi.fn(async () => false);
    setDiscardPrompt(prompt);
    guardUnsavedChanges(() => false);
    expect(await confirmDiscardChanges()).toBe(true);
    expect(prompt).not.toHaveBeenCalled();

    guardUnsavedChanges(() => true);
    expect(hasUnsavedChanges()).toBe(true);
    expect(await confirmDiscardChanges()).toBe(false);
    expect(prompt).toHaveBeenCalledOnce();
  });

  it("uses the same guard for browser unloads", () => {
    const stop = startUnloadGuard();
    guardUnsavedChanges(() => true);
    const event = new Event("beforeunload", { cancelable: true });
    window.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    stop();
  });
});
