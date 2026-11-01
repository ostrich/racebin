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

  it("does not let a stale component unregister the current form's guard", () => {
    const stale = guardUnsavedChanges(() => true);
    guardUnsavedChanges(() => false);
    stale.unregister();
    expect(hasUnsavedChanges()).toBe(false);
  });

  it("shares one pending confirmation and keeps guarding after acceptance", async () => {
    let answer!: (accepted: boolean) => void;
    const prompt = vi.fn(
      () =>
        new Promise<boolean>((resolve) => {
          answer = resolve;
        })
    );
    setDiscardPrompt(prompt);
    guardUnsavedChanges(() => true);
    const first = confirmDiscardChanges();
    const second = confirmDiscardChanges();
    await Promise.resolve();
    expect(prompt).toHaveBeenCalledOnce();
    answer(true);
    expect(await first).toBe(true);
    expect(await second).toBe(true);
    expect(hasUnsavedChanges()).toBe(true);
  });

  it("does not apply an old form's discard response to a replacement form", async () => {
    let answer!: (accepted: boolean) => void;
    setDiscardPrompt(
      () =>
        new Promise<boolean>((resolve) => {
          answer = resolve;
        })
    );
    guardUnsavedChanges(() => true);
    const pending = confirmDiscardChanges();
    await Promise.resolve();
    guardUnsavedChanges(() => true);
    answer(true);
    expect(await pending).toBe(false);
    expect(hasUnsavedChanges()).toBe(true);
  });
});
