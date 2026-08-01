import type { Paste } from "./types";

export function formatDate(value: number | string | null): string {
  const timestamp = typeof value === "string" ? Date.parse(value) : value ? value * 1000 : 0;
  return timestamp
    ? new Intl.DateTimeFormat(undefined, {
        dateStyle: "medium",
        timeStyle: "short"
      }).format(timestamp)
    : "Never";
}

export function pasteDisplayTitle(paste: Paste): string {
  return paste.title || paste.id;
}

export function pasteFormatLabel(paste: Paste): string {
  if (paste.content_kind === "rich_text") return "Rich text";
  return paste.language;
}

export function formatByteSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KiB`;
  }
  return `${(bytes / 1024 / 1024).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0)} MiB`;
}
