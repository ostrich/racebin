import { describe, expect, it } from "vitest";
import { formatByteSize, pasteDisplayTitle, pasteFormatLabel } from "./format";
import type { Paste } from "./types";

const paste = {
  format: "text",
  language: "javascript",
  content: ""
} as Paste;

describe("paste formatting", () => {
  it("uses a rich-text label instead of plaintext", () => {
    expect(pasteFormatLabel({ ...paste, format: "markdown" })).toBe("Rich text");
    expect(pasteFormatLabel(paste)).toBe("javascript");
  });

  it("formats total byte sizes consistently", () => {
    expect(formatByteSize(900)).toBe("900 B");
    expect(formatByteSize(1536)).toBe("1.5 KiB");
    expect(formatByteSize(2 * 1024 * 1024)).toBe("2.0 MiB");
  });

  it("uses attachment context for otherwise untitled pastes", () => {
    expect(pasteDisplayTitle({
      ...paste, title: "", attachment_count: 0, attachments: []
    })).toBe("Untitled");
    expect(pasteDisplayTitle({
      ...paste,
      title: "",
      attachment_count: 1,
      attachment_only_filename: "report.pdf",
      attachments: []
    })).toBe("report.pdf");
    expect(pasteDisplayTitle({
      ...paste,
      title: "",
      attachment_count: 3,
      attachment_only_filename: "report.pdf",
      attachments: []
    })).toBe("report.pdf and 2 more");
  });

  it("always prefers an explicit title", () => {
    expect(pasteDisplayTitle({
      ...paste,
      title: "Release notes",
      attachment_count: 2,
      attachment_only_filename: "report.pdf",
      attachments: []
    })).toBe("Release notes");
  });

  it("does not name a text paste after a companion attachment", () => {
    expect(pasteDisplayTitle({
      ...paste,
      title: "",
      content: "Paste body",
      attachment_count: 1,
      attachment_only_filename: "report.pdf",
      attachments: []
    })).toBe("Untitled");
  });
});
