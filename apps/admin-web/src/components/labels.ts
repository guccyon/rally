import type { Schemas } from "@/api/client";

// 契約の enum の値 → 画面の日本語。

export const SPORT_LABELS: Record<Schemas["Sport"], string> = {
  tennis: "テニス",
  golf: "ゴルフ",
  baseball: "野球",
  soccer: "サッカー",
  basketball: "バスケットボール",
};

export const SLOT_STATUS_LABELS: Record<Schemas["SlotStatus"], string> = {
  open: "公開中",
  booked: "予約済み",
  closed: "終了",
};

export const BOOKING_STATUS_LABELS: Record<Schemas["BookingStatus"], string> = {
  pending: "確定待ち",
  confirmed: "確定済み",
  cancelled: "キャンセル済み",
};

export const SESSION_STATUS_LABELS: Record<Schemas["LiveLessonSessionStatus"], string> = {
  in_progress: "通話中",
  ended: "終了",
};

export const SLOT_SUPPLY_STATUS_LABELS: Record<Schemas["SlotSupplyStatus"], string> = {
  sufficient: "足りている",
  insufficient: "足りていない",
};

/** 契約の date-time を日本時間で表示する。 */
export function formatDateTime(value: string): string {
  return new Date(value).toLocaleString("ja-JP", { timeZone: "Asia/Tokyo" });
}
