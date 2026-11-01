export type DiscardPrompt = () => Promise<boolean>;

type GuardOwner = { token: symbol; check: () => boolean };
export type DirtyFormGuard = { disarm: () => void; unregister: () => void };

type GuardState =
  | { phase: "idle" }
  | { phase: "armed"; owner: GuardOwner }
  | { phase: "confirming"; owner: GuardOwner; response: Promise<boolean> };

let state: GuardState = { phase: "idle" };
let discardPrompt: DiscardPrompt = async () => false;

export function setDiscardPrompt(prompt: DiscardPrompt): void {
  discardPrompt = prompt;
}

/** Registers the single form owned by the current route. */
export function guardUnsavedChanges(check: () => boolean): DirtyFormGuard {
  const owner = { token: Symbol("dirty form"), check };
  state = { phase: "armed", owner };
  const release = () => {
    if (state.phase !== "idle" && state.owner.token === owner.token) state = { phase: "idle" };
  };
  return { disarm: release, unregister: release };
}

export function clearUnsavedChangesGuard(): void {
  state = { phase: "idle" };
}

export function hasUnsavedChanges(): boolean {
  return state.phase !== "idle" && state.owner.check();
}

export async function confirmDiscardChanges(): Promise<boolean> {
  if (state.phase === "idle" || !state.owner.check()) return true;
  if (state.phase === "confirming") return state.response;
  const owner = state.owner;
  const response = Promise.resolve()
    .then(discardPrompt)
    .then((accepted) => state.phase !== "idle" && state.owner === owner && accepted)
    .finally(() => {
      if (state.phase === "confirming" && state.owner === owner) state = { phase: "armed", owner };
    });
  state = { phase: "confirming", owner, response };
  return response;
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
