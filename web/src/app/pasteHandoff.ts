import type { Paste } from "../types";

const pending = new Map<string, Paste>();

export function stagePasteHandoff(paste: Paste): void {
  pending.clear();
  pending.set(paste.id, paste);
}

export function takePasteHandoff(pasteId: string): Paste | undefined {
  const paste = pending.get(pasteId);
  pending.delete(pasteId);
  return paste;
}
