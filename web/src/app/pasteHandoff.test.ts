import { describe, expect, it } from "vitest";
import { paste } from "../../e2e/support/mockApi";
import type { Paste } from "../types";
import { stagePasteHandoff, takePasteHandoff } from "./pasteHandoff";

const resource: Paste = {
  ...paste,
  source_url: paste.source_url ?? undefined,
  visibility: "unlisted",
  created_at: Date.parse(paste.created_at) / 1000,
  updated_at: Date.parse(paste.updated_at) / 1000
};

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
