import type { Schemas } from "@/api/client";

// 契約の enum の値 → 画面の日本語。

export const SPORT_LABELS: Record<Schemas["Sport"], string> = {
  tennis: "テニス",
  golf: "ゴルフ",
  baseball: "野球",
  soccer: "サッカー",
  basketball: "バスケットボール",
  swimming: "水泳",
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

/** glossary の live_lesson（同期レッスン）と async_lesson（非同期レッスン）。 */
export const LESSON_TYPE_LABELS: Record<Schemas["LessonType"], string> = {
  live_lesson: "同期（ビデオ通話）",
  async_lesson: "非同期（動画の添削）",
};

export const SLOT_SUPPLY_STATUS_LABELS: Record<Schemas["SlotSupplyStatus"], string> = {
  sufficient: "足りている",
  insufficient: "足りていない",
};

/**
 * スタンプの絵柄（絵文字）と添える短い言葉。契約の `StampId` の画像と文言はクライアントが持つ（言葉は契約の説明の呼び名、
 * 絵文字は student-app と同じ）。選ぶ欄はこの順に並べる。
 */
export const STAMPS: Record<Schemas["StampId"], { emoji: string; label: string }> = {
  thanks: { emoji: "🙏", label: "ありがとう" },
  good: { emoji: "👍", label: "いいね" },
  got_it: { emoji: "👌", label: "了解" },
  lets_go: { emoji: "💪", label: "がんばろう" },
  nice_shot: { emoji: "🎯", label: "ナイスショット" },
  tired: { emoji: "🍵", label: "おつかれさま" },
  question: { emoji: "❓", label: "質問" },
  ok: { emoji: "🙆", label: "OK" },
};

/** このアプリが知っているスタンプの絵柄と言葉。ID がない・知らない ID（契約より新しい版のスタンプ）なら undefined。 */
export function knownStamp(stampId: string | undefined) {
  return stampId !== undefined && Object.hasOwn(STAMPS, stampId)
    ? STAMPS[stampId as Schemas["StampId"]]
    : undefined;
}

/** 契約の date-time を日本時間で表示する。 */
export function formatDateTime(value: string): string {
  return new Date(value).toLocaleString("ja-JP", { timeZone: "Asia/Tokyo" });
}
