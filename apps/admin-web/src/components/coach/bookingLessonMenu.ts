import type { Schemas } from "@/api/client";

/**
 * 予約した時点のレッスンメニュー（契約の `Booking.lesson_menu`）の表示。名前と 1 回の時間を「名前（60 分）」の形で
 * （枠の公開のメニューの選択肢と同じ形）、時間がない（予約した時点で非同期に直されていた）ときは名前だけ、
 * メニューのない予約は「—」（自分の枠の一覧と同じ）。
 */
export function bookingLessonMenuLabel(menu: Schemas["BookingLessonMenu"] | null | undefined): string {
  if (!menu) return "—";
  return menu.duration_minutes === null ? menu.name : `${menu.name}（${menu.duration_minutes} 分）`;
}
