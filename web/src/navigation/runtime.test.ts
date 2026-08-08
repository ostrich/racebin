import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import { clearUnsavedChangesGuard, setDiscardPrompt } from "./guards";
import { holdNavigation, locationState, navigate, navigationReady, startNavigation } from "./runtime";

describe("navigation runtime", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = "<main><h1>Page heading</h1></main>";
    history.replaceState({}, "", "/");
    navigationReady.set(false);
    clearUnsavedChangesGuard();
    setDiscardPrompt(async () => true);
    Object.defineProperty(window, "scrollX", { configurable: true, value: 0 });
    Object.defineProperty(window, "scrollY", { configurable: true, value: 0 });
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    vi.stubGlobal("scrollTo", vi.fn());
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("resolves access policy before committing the visible route", async () => {
    history.replaceState({}, "", "/pastes");
    const seen: string[] = [];
    const unsubscribe = locationState.subscribe(value => seen.push(value.path));
    const stop = await startNavigation({
      accessPolicy: location => location.route.name === "my-pastes" ? "/login" : null,
      siteName: () => "Test Racebin"
    });

    expect(get(locationState).route).toEqual({ name: "login" });
    expect(location.pathname).toBe("/login");
    expect(seen).not.toContain("/pastes");
    expect(document.title).toBe("Log in · Test Racebin");
    unsubscribe();
    stop();
  });

  it("waits for the mounted page's readiness hold before completing", async () => {
    let release = () => {};
    let markHeld = () => {};
    const held = new Promise<void>(resolve => { markHeld = resolve; });
    const unsubscribe = locationState.subscribe(value => {
      if (value.path === "/help") {
        release = holdNavigation();
        markHeld();
      }
    });
    const stop = await startNavigation();
    let completed = false;
    const pending = navigate("/help").then(result => { completed = result; });
    await held;
    expect(completed).toBe(false);
    release();
    await pending;
    expect(completed).toBe(true);
    unsubscribe();
    stop();
  });

  it("prevents a slow superseded policy decision from overwriting a newer route", async () => {
    let releaseSlow = () => {};
    const slowPolicy = new Promise<void>(resolve => { releaseSlow = resolve; });
    const stop = await startNavigation({ accessPolicy: async location => {
      if (location.path === "/explore") await slowPolicy;
      return null;
    }});
    const slow = navigate("/explore");
    const latest = navigate("/help");
    releaseSlow();
    expect(await slow).toBe(false);
    expect(await latest).toBe(true);
    expect(location.pathname).toBe("/help");
    stop();
  });

  it("keeps history writes off the active scroll path and checkpoints after scrolling settles", async () => {
    const stop = await startNavigation();
    const replaceState = vi.spyOn(history, "replaceState");
    Object.defineProperty(window, "scrollX", { configurable: true, value: 12 });
    Object.defineProperty(window, "scrollY", { configurable: true, value: 480 });

    window.dispatchEvent(new Event("scroll"));
    vi.advanceTimersByTime(150);
    window.dispatchEvent(new Event("scroll"));
    vi.advanceTimersByTime(199);
    expect(replaceState).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(replaceState).toHaveBeenCalledTimes(1);
    expect(history.state.racebin.scroll).toEqual({ x: 12, y: 480 });
    stop();
  });

  it("flushes a pending scroll position before programmatic navigation", async () => {
    const stop = await startNavigation();
    const replaceState = vi.spyOn(history, "replaceState");
    Object.defineProperty(window, "scrollX", { configurable: true, value: 0 });
    Object.defineProperty(window, "scrollY", { configurable: true, value: 720 });
    window.dispatchEvent(new Event("scroll"));

    await navigate("/help");

    expect(replaceState.mock.calls[0]?.[0]).toMatchObject({
      racebin: { index: 0, scroll: { x: 0, y: 720 } }
    });
    stop();
  });
});
