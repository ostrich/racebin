import { describe, expect, it } from "vitest";
import { paste } from "../../e2e/support/mockApi";
import { pasteFromWire } from "../api/normalize";
import { stagePasteHandoff, takePasteHandoff } from "./pasteHandoff";

const resource = pasteFromWire(paste);

describe("paste navigation handoff", () => {
  it("transfers an authoritative mutation result exactly once", () => {
    stagePasteHandoff(resource);
    expect(takePasteHandoff(resource.id)).toBe(resource);
    expect(takePasteHandoff(resource.id)).toBeUndefined();
  });

  it("discards an older staged resource", () => {
    stagePasteHandoff(resource);
    stagePasteHandoff({ ...resource, id: "newer" });
    expect(takePasteHandoff(resource.id)).toBeUndefined();
    expect(takePasteHandoff("newer")?.id).toBe("newer");
  });
});
