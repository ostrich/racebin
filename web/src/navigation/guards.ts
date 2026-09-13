export type DiscardPrompt = () => Promise<boolean>;

type GuardOwner = { token: symbol; check: () => boolean };
export type DirtyFormGuard = { disarm: () => void; unregister: () => void };

let unsavedGuard: GuardOwner | undefined;
let discardPrompt: DiscardPrompt = async () => false;

export function setDiscardPrompt(prompt: DiscardPrompt): void {
  discardPrompt = prompt;
}

/** Registers the single form owned by the current route. */
export function guardUnsavedChanges(check: () => boolean): DirtyFormGuard {
  const owner = { token: Symbol("dirty form"), check };
  unsavedGuard = owner;
  const release = () => {
    if (unsavedGuard?.token === owner.token) unsavedGuard = undefined;
  };
  return { disarm: release, unregister: release };
}

export function clearUnsavedChangesGuard(): void {
  unsavedGuard = undefined;
}

export function hasUnsavedChanges(): boolean {
  return unsavedGuard?.check() ?? false;
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
