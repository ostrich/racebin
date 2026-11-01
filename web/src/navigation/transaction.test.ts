import { describe, expect, it, vi } from "vitest";
import { NavigationTransaction } from "./transaction";

describe("navigation transaction phases", () => {
  it("waits for sealing and every owned hold before finalization", async () => {
    const transaction = new NavigationTransaction();
    const ready = vi.fn();
    void transaction.ready.then(ready);
    expect(transaction.state).toBe("resolving");
    transaction.publish();
    const first = transaction.hold();
    const second = transaction.hold();
    first();
    first();
    transaction.seal();
    expect(transaction.state).toBe("waiting");
    await Promise.resolve();
    expect(ready).not.toHaveBeenCalled();
    second();
    await transaction.ready;
    expect(transaction.state).toBe("finalizing");
    transaction.complete();
    expect(transaction.state).toBe("complete");
    expect(() => transaction.publish()).toThrow();
  });

  it("cancellation releases waiters without resurrecting the transaction", async () => {
    const transaction = new NavigationTransaction();
    transaction.publish();
    const release = transaction.hold();
    transaction.seal();
    transaction.cancel();
    await transaction.ready;
    release();
    transaction.cancel();
    expect(transaction.state).toBe("cancelled");
    expect(() => transaction.complete()).toThrow();
  });

  it("cannot register readiness after sealing or complete before readiness", () => {
    const transaction = new NavigationTransaction();
    expect(() => transaction.seal()).toThrow();
    expect(() => transaction.complete()).toThrow();
    transaction.publish();
    transaction.seal();
    transaction.hold()();
    transaction.complete();
    expect(transaction.state).toBe("complete");
  });
});
