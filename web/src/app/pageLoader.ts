import { holdNavigation } from "../navigation";

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
export function createPageLoader() {
  let generation = 0;
  let initialNavigationRelease: (() => void) | undefined = holdNavigation();

  function load<T>(request: () => Promise<T>, callbacks: PageLoadCallbacks<T>): () => void {
    const current = ++generation;
    const releaseNavigation = initialNavigationRelease ?? holdNavigation();
    initialNavigationRelease = undefined;
    let active = true;

    void request()
      .then((value) => {
        if (active && current === generation) callbacks.success(value);
      })
      .catch((reason: unknown) => {
        if (active && current === generation) callbacks.failure(reason);
      })
      .finally(() => {
        if (active && current === generation) callbacks.settled?.();
        releaseNavigation();
      });

    return () => {
      active = false;
      if (current === generation) generation += 1;
      releaseNavigation();
    };
  }

  return { load };
}
