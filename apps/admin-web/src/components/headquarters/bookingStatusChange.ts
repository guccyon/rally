// 本部の「予約の詳細」の「状態が変わった記録」のキャンセルの経緯を、CS が生徒に説明できる文にする
// （specs/ux/issue-1030.yaml の hqBookingDetail の elements）。状態のコード・ID・true/false は画面に出さない。

import type { Schemas } from "@/api/client";
import { BOOKING_CANCELLER_LABELS } from "@/components/labels";

type BookingStatusChange = Schemas["BookingStatusChange"];

/** 経緯の値の記録がないときの表記。推測で埋めない。 */
export const NO_RECORD = "記録がありません";

/** 理由がコーチのお休みか。未知の理由は理由の記録なしと同じに扱う（契約の BookingCancelReason）。 */
export function isCoachLeaveCancel(change: BookingStatusChange): boolean {
  return change.cancel_reason === "coach_leave";
}

/** 経緯を残し始める前のキャンセル（キャンセルした人と枠の判定がどちらもない）。 */
export function isLegacyCancel(change: BookingStatusChange): boolean {
  return change.cancelled_by == null && change.consumes_quota == null;
}

/** キャンセルした人。 */
export function cancellerLabel(change: BookingStatusChange): string {
  switch (change.cancelled_by) {
    case "student":
      return `${BOOKING_CANCELLER_LABELS.student}（自分でキャンセル）`;
    case "coach":
      return BOOKING_CANCELLER_LABELS.coach;
    case "headquarters":
      return isCoachLeaveCancel(change)
        ? `${BOOKING_CANCELLER_LABELS.headquarters}（コーチのお休み）`
        : BOOKING_CANCELLER_LABELS.headquarters;
    default:
      return NO_RECORD;
  }
}

/** 開始の何時間前か（契約の hours_before_start。端数は 0 に向けて切り捨て、開始より後は負）。 */
export function hoursBeforeStartLabel(hours: number | undefined): string {
  if (hours === undefined) return NO_RECORD;
  if (hours < 0) return `開始の ${-hours} 時間後`;
  if (hours === 0) return "開始の 1 時間以内";
  if (hours < 24) return `開始の ${hours} 時間前`;
  const [days, rest] = [Math.floor(hours / 24), hours % 24];
  return `開始の ${hours} 時間前（${days} 日${rest === 0 ? "" : ` ${rest} 時間`}前）`;
}

/** ビデオ通話の枠を使ったか。consumes_quota を正とし、開始の何時間前かから推さない。 */
export function quotaLabel(change: BookingStatusChange): string {
  if (change.consumes_quota == null) return NO_RECORD;
  if (change.consumes_quota) return "1 回使いました（キャンセル規定: 開始 24 時間前より後のキャンセル）";
  if (isCoachLeaveCancel(change)) return "使っていません（コーチのお休みのキャンセルは回数に数えません）";
  if (change.cancelled_by === "student") return "使っていません（開始 24 時間前までのキャンセル）";
  return "使っていません";
}

/**
 * 理由。生徒・コーチのキャンセルは理由を保存しない決まりなので、理由の記録がなければ行ごと出さない（undefined）。
 */
export function reasonLabel(change: BookingStatusChange): string | undefined {
  if (isCoachLeaveCancel(change)) return "コーチのお休み";
  if (change.cancelled_by === "headquarters") return `${NO_RECORD}（本部のキャンセルで、理由が残っていません）`;
  if (change.cancelled_by == null) return NO_RECORD;
  return undefined;
}
