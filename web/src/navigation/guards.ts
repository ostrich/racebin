export type DiscardPrompt = () => Promise<boolean>;

let unsavedCheck: (() => boolean) | undefined;
let discardPrompt: DiscardPrompt = async () => window.confirm("Discard unsaved changes?");

export function setDiscardPrompt(prompt: DiscardPrompt): void {
  discardPrompt = prompt;
}

/** Registers the single form owned by the current route. */
export function guardUnsavedChanges(check?: () => boolean): void {
  unsavedCheck = check;
}

export function clearUnsavedChangesGuard(): void {
  unsavedCheck = undefined;
}

export function hasUnsavedChanges(): boolean {
  return unsavedCheck?.() ?? false;
}

export async function confirmDiscardChanges(): Promise<boolean> {
  return !hasUnsavedChanges() || discardPrompt();
}

export function startUnloadGuard(): () => void {
  const beforeUnload = (event: BeforeUnloadEvent) => {
    if (!hasUnsavedChanges()) return;
    event.preventDefault();
    event.returnValue = "";
  };
  window.addEventListener("beforeunload", beforeUnload);
  return () => window.removeEventListener("beforeunload", beforeUnload);
}
