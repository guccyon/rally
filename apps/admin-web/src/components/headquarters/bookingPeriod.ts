// 本部の予約の一覧（listBookings）の期間と、開始が近い確定待ちの見分け。日付は日本時間（契約の `from` / `to` と同じ）。

const DAY = 86_400_000;
/** 日本時間は UTC+9（夏時間はない）。 */
const JST_OFFSET = 9 * 3_600_000;

/** 期間の日数の上限（契約の `listBookings`。`from` と `to` を含めて数える）。 */
export const MAX_PERIOD_DAYS = 31;

/** 既定の期間の日数（契約で `to` を省いたときと同じく、`from` を含めて 7 日）。 */
const DEFAULT_PERIOD_DAYS = 7;

/** 確定待ちのまま開始までこの時間を切った予約を目立たせる（キャンセル規定の境目と同じ 24 時間）。 */
export const PENDING_SOON_MS = DAY;

export type Period = { from: string; to: string };

/** その時刻の日本時間の日付（YYYY-MM-DD）。 */
export function jstDate(epochMs: number): string {
  return new Date(epochMs + JST_OFFSET).toISOString().slice(0, 10);
}

/** 既定の期間: 今日（日本時間）から 7 日。 */
export function defaultPeriod(now: number): Period {
  return { from: jstDate(now), to: jstDate(now + (DEFAULT_PERIOD_DAYS - 1) * DAY) };
}

/**
 * 送る前に期間を確かめる（契約の 422 と同じ条件）。正しければ null、正しくなければ理由の文言。
 * サーバーの 422 は ErrorCode の汎用の文言しか出せないので、どこを直すかはここで伝える。
 */
export function periodError({ from, to }: Period): string | null {
  if (from === "" || to === "") return "開始日と終了日を選んでください。";
  const days = (Date.parse(to) - Date.parse(from)) / DAY + 1;
  if (Number.isNaN(days)) return "開始日と終了日を選んでください。";
  if (days < 1) return "終了日は開始日と同じか、それより後の日付にしてください。";
  if (days > MAX_PERIOD_DAYS) return `期間は ${MAX_PERIOD_DAYS} 日までにしてください。`;
  return null;
}

/** 確定待ち（`pending`）のまま、まだ始まっておらず、開始まで 24 時間を切っている予約か。 */
export function isPendingSoon(booking: { status: string; starts_at: string }, now: number): boolean {
  if (booking.status !== "pending") return false;
  const untilStart = Date.parse(booking.starts_at) - now;
  return untilStart > 0 && untilStart <= PENDING_SOON_MS;
}
