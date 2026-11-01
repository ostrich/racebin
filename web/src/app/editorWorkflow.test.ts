import { describe, expect, it } from "vitest";
import { isSaving, transitionSave, type SaveState } from "./editorWorkflow";
import type { Paste } from "../types";

const paste: Paste = {
  id: "example",
  owner_id: 1,
  folder_id: null,
  title: "Example",
  content: "text",
  rendered_html: null,
  plain_text: "text",
  format: "text",
  language: "plaintext",
  visibility: "private",
  created_at: 1,
  modified_at: null,
  expires_at: null,
  last_read_at: null,
  read_count: 0,
  read_limit: null,
  attachment_count: 0,
  size_bytes: 4,
  attachments: []
};

describe("editor save workflow", () => {
  it("completes atomic creation without a separate upload phase", () => {
    const saving = transitionSave({ phase: "idle" }, { type: "start" });
    expect(isSaving(saving)).toBe(true);
    const complete = transitionSave(saving, {
      type: "content-saved",
      paste,
      uploadAttachments: false
    });
    expect(complete).toEqual({ phase: "complete", savedPaste: paste });
  });

  it("retains committed content through attachment failure and permits retry", () => {
    let state: SaveState = transitionSave({ phase: "idle" }, { type: "start" });
    state = transitionSave(state, { type: "content-saved", paste, uploadAttachments: true });
    state = transitionSave(state, { type: "attachments-failed", message: "offline" });
    expect(state).toEqual({ phase: "attachments-failed", savedPaste: paste, message: "offline" });
    expect(isSaving(state)).toBe(false);
    state = transitionSave(state, { type: "start" });
    state = transitionSave(state, { type: "content-saved", paste, uploadAttachments: true });
    state = transitionSave(state, { type: "attachments-saved" });
    expect(state.phase).toBe("complete");
  });

  it("rejects duplicate saves and upload completion before content is committed", () => {
    const saving = transitionSave({ phase: "idle" }, { type: "start" });
    expect(() => transitionSave(saving, { type: "start" })).toThrow();
    expect(() => transitionSave(saving, { type: "attachments-saved" })).toThrow();
    expect(transitionSave(saving, { type: "failed", message: "offline" }).phase).toBe("failed");
  });
});
