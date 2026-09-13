import { onDestroy } from "svelte";
import { guardUnsavedChanges, type DirtyFormGuard } from "../navigation";

/** Owns an unsaved-change guard for exactly one component lifetime. */
export function useDirtyForm(isDirty: () => boolean): DirtyFormGuard {
  const guard = guardUnsavedChanges(isDirty);
  onDestroy(guard.unregister);
  return guard;
}
