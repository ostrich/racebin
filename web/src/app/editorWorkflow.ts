import type { Paste } from "../types";

export type SaveState =
  | { phase: "idle" }
  | { phase: "saving-content" }
  | { phase: "uploading-attachments"; savedPaste: Paste }
  | { phase: "attachments-failed"; savedPaste: Paste; message: string }
  | { phase: "complete"; savedPaste: Paste }
  | { phase: "failed"; message: string };

type SaveEvent =
  | { type: "start" }
  | { type: "content-saved"; paste: Paste; uploadAttachments: boolean }
  | { type: "attachments-saved" }
  | { type: "attachments-failed"; message: string }
  | { type: "failed"; message: string };

export function isSaving(state: SaveState): boolean {
  return ["saving-content", "uploading-attachments", "complete"].includes(state.phase);
}

export function transitionSave(state: SaveState, event: SaveEvent): SaveState {
  switch (event.type) {
    case "start":
      if (!isSaving(state)) return { phase: "saving-content" };
      break;
    case "content-saved":
      if (state.phase === "saving-content")
        return {
          phase: event.uploadAttachments ? "uploading-attachments" : "complete",
          savedPaste: event.paste
        };
      break;
    case "attachments-saved":
      if (state.phase === "uploading-attachments")
        return { phase: "complete", savedPaste: state.savedPaste };
      break;
    case "attachments-failed":
      if (state.phase === "uploading-attachments")
        return {
          phase: "attachments-failed",
          savedPaste: state.savedPaste,
          message: event.message
        };
      break;
    case "failed":
      if (isSaving(state)) return { phase: "failed", message: event.message };
      break;
  }
  throw new Error(`Invalid save transition: ${state.phase} → ${event.type}`);
}

type Format = "text" | "markdown";
export type ConversionState =
  | { phase: "idle" }
  | { phase: "converting"; source: Format; target: Format; original: string }
  | { phase: "confirming"; source: Format; target: Format; original: string; preview: string }
  | { phase: "validating-visual"; original: string };
