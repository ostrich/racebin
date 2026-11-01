import { holdNavigation } from "../navigation";
import { onDestroy } from "svelte";

type PageLoadCallbacks<T> = {
  success: (value: T) => void;
  failure: (reason: unknown) => void;
  settled?: () => void;
};

/**
 * Coordinates replaceable page requests with the active navigation.
 * Superseded requests may finish, but cannot publish data, errors, or loading
 * state into the current page.
 */
export function createPageLoader(registerDisposal: (dispose: () => void) => void = onDestroy) {
  let generation = 0;
  let lifecycle: "active" | "disposed" = "active";
  let initialNavigationRelease: (() => void) | undefined = holdNavigation();
  const activeLoads = new Set<() => void>();

  function dispose(): void {
    lifecycle = "disposed";
    generation += 1;
    initialNavigationRelease?.();
    initialNavigationRelease = undefined;
    for (const cancel of [...activeLoads]) cancel();
  }

  registerDisposal(dispose);

  function load<T>(request: () => Promise<T>, callbacks: PageLoadCallbacks<T>): () => void {
    if (lifecycle === "disposed") return () => {};
    const current = ++generation;
    const releaseNavigation = initialNavigationRelease ?? holdNavigation();
    initialNavigationRelease = undefined;
    let phase: "pending" | "cancelled" | "settled" = "pending";
    const cancel = () => {
      if (phase !== "pending") return;
      phase = "cancelled";
      activeLoads.delete(cancel);
      if (current === generation) generation += 1;
      releaseNavigation();
    };
    activeLoads.add(cancel);

    void request()
      .then((value) => {
        if (phase === "pending" && current === generation) callbacks.success(value);
      })
      .catch((reason: unknown) => {
        if (phase === "pending" && current === generation) callbacks.failure(reason);
      })
      .finally(() => {
        try {
          if (phase === "pending" && current === generation) callbacks.settled?.();
        } finally {
          if (phase === "pending") releaseNavigation();
          phase = "settled";
          activeLoads.delete(cancel);
        }
      });

    return cancel;
  }

  return { load, dispose };
}
