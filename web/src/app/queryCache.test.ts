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

  it("does not restore an in-flight result after invalidation", async () => {
    let resolveRequest: (value: { count: number }) => void = () => {};
    const pending = loadQuery("/items", () => new Promise(resolve => {
      resolveRequest = resolve;
    }));
    clearQueryCache();
    resolveRequest({ count: 2 });
    await pending;
    expect(cachedQuery("/items")).toBeUndefined();
  });
});
