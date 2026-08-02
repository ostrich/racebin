import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import { clearUnsavedChangesGuard, setDiscardPrompt } from "./guards";
import { holdNavigation, locationState, navigate, navigationReady, startNavigation } from "./runtime";

describe("navigation runtime", () => {
  beforeEach(() => {
    document.body.innerHTML = "<main><h1>Page heading</h1></main>";
    history.replaceState({}, "", "/");
    navigationReady.set(false);
    clearUnsavedChangesGuard();
    setDiscardPrompt(async () => true);
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    vi.stubGlobal("scrollTo", vi.fn());
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
});
