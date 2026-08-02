export type ScrollPosition = { x: number; y: number };

type NavigationHistoryState = {
  racebin?: { index: number; scroll: ScrollPosition };
  [key: string]: unknown;
};

export function historyState(state: unknown = history.state): NavigationHistoryState {
  return state && typeof state === "object" ? state as NavigationHistoryState : {};
}

export function historyIndex(state: unknown = history.state): number | undefined {
  const index = historyState(state).racebin?.index;
  return typeof index === "number" ? index : undefined;
}

export function savedScroll(state: unknown = history.state): ScrollPosition {
  const scroll = historyState(state).racebin?.scroll;
  return {
    x: typeof scroll?.x === "number" ? scroll.x : 0,
    y: typeof scroll?.y === "number" ? scroll.y : 0
  };
}

export function stateWithNavigation(
  index: number,
  scroll: ScrollPosition,
  state: unknown = history.state
): NavigationHistoryState {
  return { ...historyState(state), racebin: { index, scroll } };
}

export function currentScroll(): ScrollPosition {
  return { x: window.scrollX, y: window.scrollY };
}

export function replaceSavedScroll(index: number, scroll = currentScroll()): void {
  history.replaceState(stateWithNavigation(index, scroll), "");
}

export function restoreScroll(position: ScrollPosition): void {
  window.scrollTo(position.x, position.y);
}
