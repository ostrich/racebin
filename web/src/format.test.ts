import { describe, expect, it } from "vitest";
import { formatByteSize, pasteFormatLabel } from "./format";
import type { Paste } from "./types";

const paste = {
  content_kind: "text",
  language: "javascript"
} as Paste;

describe("paste formatting", () => {
  it("uses a rich-text label instead of plaintext", () => {
    expect(pasteFormatLabel({ ...paste, content_kind: "rich_text" })).toBe("Rich text");
    expect(pasteFormatLabel(paste)).toBe("javascript");
  });

  it("formats total byte sizes consistently", () => {
    expect(formatByteSize(900)).toBe("900 B");
    expect(formatByteSize(1536)).toBe("1.5 KiB");
    expect(formatByteSize(2 * 1024 * 1024)).toBe("2.0 MiB");
  });
});
