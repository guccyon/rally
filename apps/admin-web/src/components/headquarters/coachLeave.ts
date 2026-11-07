// 本部の「コーチのお休み」の期間の確かめと、日時・期間の表示（specs/ux/issue-455.yaml の hqCoachLeave）。
// 日付は日本時間（契約の `getCoachLeavePreview` / `startCoachLeave` の `from` / `to` と同じ）。

import { toJstDate } from "@/components/labels";

import { MAX_PERIOD_DAYS } from "./bookingPeriod";

const DAY = 86_400_000;

/** 生徒への一言の上限（契約の `StartCoachLeaveRequest.note` の maxLength。コードポイントで数える）。 */
export const NOTE_MAX_CHARS = 200;

export type LeavePeriod = { from: string; to: string };

/** 期間の誤り。欄ごとの文言（なければ undefined）。 */
export type LeavePeriodErrors = { from?: string; to?: string };

/** 期間を確かめる（契約の 422 と同じ条件。始まりは明日以降・終わりは始まりと同じか後・31 日まで）。`now` は epoch ミリ秒。 */
export function leavePeriodErrors({ from, to }: LeavePeriod, now: number): LeavePeriodErrors {
  const errors: LeavePeriodErrors = {};
  const today = toJstDate(now);
  if (from === "") errors.from = "始まりの日を選んでください。";
  else if (from <= today) errors.from = "始まりの日は明日以降の日にしてください。";
  if (to === "") errors.to = "終わりの日を選んでください。";
  else if (from !== "") {
    const days = (Date.parse(to) - Date.parse(from)) / DAY + 1;
    if (days < 1) errors.to = "終わりの日は始まりの日と同じか、それより後の日にしてください。";
    else if (days > MAX_PERIOD_DAYS) errors.to = `お休みの期間は ${MAX_PERIOD_DAYS} 日までにしてください。`;
  }
  return errors;
}

/** 文字数（コードポイントで数える。受付の停止の理由と同じ）。 */
export function countChars(value: string): number {
  return Array.from(value).length;
}

const WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];

const JST_PARTS = new Intl.DateTimeFormat("en-US", {
  timeZone: "Asia/Tokyo",
  month: "numeric",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
  hourCycle: "h23",
  weekday: "short",
});

const WEEKDAY_INDEX: Record<string, number> = { Sun: 0, Mon: 1, Tue: 2, Wed: 3, Thu: 4, Fri: 5, Sat: 6 };

function jst(value: string) {
  const parts = JST_PARTS.formatToParts(new Date(value));
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find((p) => p.type === type)?.value ?? "";
  return {
    day: `${part("month")}/${part("day")}（${WEEKDAYS[WEEKDAY_INDEX[part("weekday")] ?? 0]}）`,
    time: `${Number(part("hour"))}:${part("minute")}`,
  };
}

/** 枠の日時（「11/10（火）19:00〜19:30」。日本時間）。 */
export function slotTimeLabel(startsAt: string, endsAt: string): string {
  const start = jst(startsAt);
  return `${start.day}${start.time}〜${jst(endsAt).time}`;
}

/** 契約の日付（YYYY-MM-DD）を「11/10（火）」の形にする。形が違えばそのまま出す。 */
export function dayLabel(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return value;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  return `${month}/${day}（${WEEKDAYS[new Date(Date.UTC(year, month - 1, day)).getUTCDay()]}）`;
}

/** 期間（「11/10（火）〜11/16（月）」）。 */
export function periodLabel({ from, to }: LeavePeriod): string {
  return `${dayLabel(from)}〜${dayLabel(to)}`;
}
