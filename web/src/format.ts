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
  if (paste.title) return paste.title;
  if (paste.content?.trim()) return "Untitled";
  const filename = paste.attachments[0]?.filename ?? paste.attachment_only_filename;
  if (!filename) return "Untitled";
  const remaining = paste.attachment_count - 1;
  return remaining > 0 ? `${filename} and ${remaining} more` : filename;
}

export function pasteFormatLabel(paste: Paste): string {
  if (paste.content_kind === "markdown") return "Rich text";
  return paste.language;
}

export function formatByteSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KiB`;
  }
  return `${(bytes / 1024 / 1024).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0)} MiB`;
}
