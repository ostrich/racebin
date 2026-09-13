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
  let initialNavigationRelease: (() => void) | undefined = holdNavigation();
  const activeLoads = new Set<() => void>();

  function dispose(): void {
    generation += 1;
    initialNavigationRelease?.();
    initialNavigationRelease = undefined;
    for (const cancel of [...activeLoads]) cancel();
  }

  registerDisposal(dispose);

  function load<T>(request: () => Promise<T>, callbacks: PageLoadCallbacks<T>): () => void {
    const current = ++generation;
    const releaseNavigation = initialNavigationRelease ?? holdNavigation();
    initialNavigationRelease = undefined;
    let active = true;
    const cancel = () => {
      if (!active) return;
      active = false;
      activeLoads.delete(cancel);
      if (current === generation) generation += 1;
      releaseNavigation();
    };
    activeLoads.add(cancel);

    void request()
      .then((value) => {
        if (active && current === generation) callbacks.success(value);
      })
      .catch((reason: unknown) => {
        if (active && current === generation) callbacks.failure(reason);
      })
      .finally(() => {
        if (active && current === generation) callbacks.settled?.();
        activeLoads.delete(cancel);
        releaseNavigation();
      });

    return cancel;
  }

  return { load, dispose };
}
