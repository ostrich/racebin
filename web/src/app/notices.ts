import { writable } from "svelte/store";

export type Notice = { id: number; message: string; variant: "default" | "error" };

export const notice = writable<Notice | null>(null);
let noticeId = 0;
let timeout: number | undefined;

export function showNotice(message: string, variant: "default" | "error" = "default"): void {
  const id = ++noticeId;
  notice.set({ id, message, variant });
  if (timeout !== undefined) window.clearTimeout(timeout);
  timeout = window.setTimeout(() => notice.update(value => value?.id === id ? null : value), 3500);
}
