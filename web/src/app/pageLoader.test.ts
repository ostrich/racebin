import { beforeEach, describe, expect, it, vi } from "vitest";

const releaseNavigation = vi.fn();
vi.mock("../navigation", () => ({
  holdNavigation: vi.fn(() => releaseNavigation)
}));

import { createPageLoader } from "./pageLoader";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

describe("page loader", () => {
  beforeEach(() => releaseNavigation.mockClear());

  it("publishes only the latest request", async () => {
    const loader = createPageLoader(() => {});
    const first = deferred<string>();
    const second = deferred<string>();
    const success = vi.fn();
    const failure = vi.fn();

    loader.load(() => first.promise, { success, failure });
    loader.load(() => second.promise, { success, failure });
    first.resolve("old");
    second.resolve("current");
    await Promise.all([first.promise, second.promise]);
    await vi.waitFor(() => expect(success).toHaveBeenCalledOnce());

    expect(success).toHaveBeenCalledWith("current");
    expect(failure).not.toHaveBeenCalled();
  });

  it("suppresses stale errors and settles navigation holds", async () => {
    const loader = createPageLoader(() => {});
    const first = deferred<string>();
    const second = deferred<string>();
    const failure = vi.fn();

    loader.load(() => first.promise, { success: vi.fn(), failure });
    loader.load(() => second.promise, { success: vi.fn(), failure });
    first.reject(new Error("stale"));
    second.resolve("current");
    await Promise.allSettled([first.promise, second.promise]);
    await vi.waitFor(() => expect(releaseNavigation).toHaveBeenCalledTimes(2));

    expect(failure).not.toHaveBeenCalled();
  });

  it("cancels publication when its owning effect is destroyed", async () => {
    const loader = createPageLoader(() => {});
    const request = deferred<string>();
    const success = vi.fn();
    const cancel = loader.load(() => request.promise, { success, failure: vi.fn() });

    cancel();
    request.resolve("late");
    await request.promise;

    expect(success).not.toHaveBeenCalled();
    expect(releaseNavigation).toHaveBeenCalledOnce();
  });

  it("owns imperative reloads until the component is destroyed", async () => {
    let destroy = () => {};
    const loader = createPageLoader((dispose) => {
      destroy = dispose;
    });
    const request = deferred<string>();
    const success = vi.fn();
    const failure = vi.fn();

    loader.load(() => request.promise, { success, failure });
    destroy();
    request.reject(new Error("late failure"));
    await request.promise.catch(() => {});

    expect(success).not.toHaveBeenCalled();
    expect(failure).not.toHaveBeenCalled();
    expect(releaseNavigation).toHaveBeenCalledOnce();
  });
});
