import { vi } from "vitest";

import type { CoachApi } from "@/api/coach";

/** お知らせのないコーチ（コーチのホームを開くほかの画面のテストの既定。見出しの未読の数が呼ぶ）。 */
export function noNotifications() {
  return vi.fn<CoachApi["listMyNotifications"]>(async () => ({
    ok: true,
    data: { notifications: [] },
  }));
}
