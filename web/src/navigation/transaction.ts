export type NavigationPhase =
  "resolving" | "publishing" | "waiting" | "finalizing" | "complete" | "cancelled";

/** One owner for readiness. Cancellation releases waiters, not a later transaction. */
export class NavigationTransaction {
  private phase: NavigationPhase = "resolving";
  private pending = 0;
  private resolve!: () => void;
  readonly ready = new Promise<void>((resolve) => {
    this.resolve = resolve;
  });

  get state(): NavigationPhase {
    return this.phase;
  }

  publish(): void {
    if (this.phase !== "resolving") throw new Error("Navigation cannot publish in this phase");
    this.phase = "publishing";
  }

  hold(): () => void {
    if (this.phase !== "publishing") return () => {};
    this.pending += 1;
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.pending -= 1;
      this.settle();
    };
  }

  seal(): void {
    if (this.phase !== "publishing") throw new Error("Navigation cannot seal in this phase");
    this.phase = "waiting";
    this.settle();
  }

  private settle(): void {
    if (this.phase === "waiting" && this.pending === 0) {
      this.phase = "finalizing";
      this.resolve();
    }
  }

  complete(): void {
    if (this.phase !== "finalizing") throw new Error("Navigation is not ready to complete");
    this.phase = "complete";
  }

  cancel(): void {
    if (this.phase === "complete" || this.phase === "cancelled") return;
    this.phase = "cancelled";
    this.resolve();
  }
}
