import { beforeEach, describe, expect, it, vi } from "vitest";
import { cachedQuery, clearQueryCache, loadQuery } from "./queryCache";

describe("query cache", () => {
  beforeEach(clearQueryCache);

  it("retains successful results for synchronous reads", async () => {
    await loadQuery("/items", async () => ({ count: 3 }));
    expect(cachedQuery("/items")).toEqual({ count: 3 });
  });

  it("deduplicates requests for the same key", async () => {
    const loader = vi.fn(async () => ({ count: 1 }));
    const [first, second] = await Promise.all([
      loadQuery("/items", loader),
      loadQuery("/items", loader)
    ]);
    expect(first).toEqual(second);
    expect(loader).toHaveBeenCalledOnce();
  });

  it("reuses a fresh response without another request", async () => {
    const loader = vi.fn(async () => ({ count: 1 }));
    await loadQuery("/items", loader);
    await loadQuery("/items", loader);
    expect(loader).toHaveBeenCalledOnce();
  });

  it("refreshes data when its freshness window has elapsed", async () => {
    const loader = vi.fn(async () => ({ count: 1 }));
    await loadQuery("/items", loader);
    await loadQuery("/items", loader, { freshForMs: 0 });
    expect(loader).toHaveBeenCalledTimes(2);
  });

  it("does not restore an in-flight result after invalidation", async () => {
    let resolveRequest: (value: { count: number }) => void = () => {};
    const pending = loadQuery(
      "/items",
      () =>
        new Promise((resolve) => {
          resolveRequest = resolve;
        })
    );
    clearQueryCache();
    resolveRequest({ count: 2 });
    await pending;
    expect(cachedQuery("/items")).toBeUndefined();
  });

  it("retains the last ready value during and after a failed refresh", async () => {
    await loadQuery("/items", async () => "cached");
    let reject!: (error: Error) => void;
    const refresh = loadQuery(
      "/items",
      () =>
        new Promise((_, fail) => {
          reject = fail;
        }),
      { freshForMs: 0 }
    );
    expect(cachedQuery("/items")).toBe("cached");
    reject(new Error("offline"));
    await expect(refresh).rejects.toThrow("offline");
    expect(cachedQuery("/items")).toBe("cached");
    expect(await loadQuery("/items", async () => "recovered", { freshForMs: 0 })).toBe("recovered");
  });
});
