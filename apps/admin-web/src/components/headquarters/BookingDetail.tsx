"use client";

import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { Card } from "@/components/ui/Card";
import { DescriptionList, type DescriptionItem } from "@/components/ui/DescriptionList";
import { EmptyState } from "@/components/ui/EmptyState";
import { SectionLoading } from "@/components/ui/Loading";
import { Notice } from "@/components/ui/Notice";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { bookingLessonMenuLabel } from "@/components/coach/bookingLessonMenu";
import {
  BOOKING_STATUS_LABELS,
  formatDateTime,
  formatShortDateTime,
  formatTime,
  toJstDate,
} from "@/components/labels";

import { BookingStatusBadge } from "./BookingStatusBadge";
import {
  cancellerLabel,
  hoursBeforeStart,
  hoursBeforeStartLabel,
  isCoachLeaveCancel,
  isLegacyCancel,
  quotaLabel,
  reasonLabel,
} from "./bookingStatusChange";
import { periodLabel } from "./coachLeave";

type HeadquartersBookingDetail = Schemas["HeadquartersBookingDetail"];
type BookingStatusChange = Schemas["BookingStatusChange"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; detail: HeadquartersBookingDetail }
  | { kind: "not_found" }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";
const NOT_FOUND = "この予約は見つかりません。予約の一覧から選び直してください。";
const NO_CHANGES = "まだ確定もキャンセルもしていない予約です。コーチが確定すると、ここに記録が出ます。";
const LEGACY_CANCEL =
  "この予約は、キャンセルの経緯を残し始める前にキャンセルされました。キャンセルした人とビデオ通話の枠を使ったかは分かりません。" +
  "開始の何時間前かは、枠の時刻とキャンセルした日時から出しています。";
const LEGACY_CANCEL_NO_HOURS =
  "この予約は、キャンセルの経緯を残し始める前にキャンセルされました。キャンセルした人とビデオ通話の枠を使ったかは分かりません。";
const LEAVE_MISSING = "お休みの記録を読み込めませんでした。開発の担当に知らせてください。";

/** 枠の時刻。終わりが開始と同じ日（日本時間）なら終わりは時刻だけにする。 */
function slotTimeLabel(slot: Schemas["Slot"]): string {
  const end = toJstDate(slot.starts_at) === toJstDate(slot.ends_at) ? formatTime(slot.ends_at) : formatDateTime(slot.ends_at);
  return `${formatDateTime(slot.starts_at)} 〜 ${end}`;
}

/** もとになったコーチのお休み（期間・生徒への一言・お休みにした本部と時刻）。 */
function CoachLeaveRecord({ leave }: { leave: Schemas["CoachLeave"] }) {
  const items: DescriptionItem[] = [
    { term: "お休みの期間", value: periodLabel(leave) },
    {
      term: "生徒への一言",
      value: <span className="block max-w-[40em]">{leave.note === "" ? "（一言はありません）" : leave.note}</span>,
    },
    {
      term: "お休みにした本部",
      value: `${leave.created_by_display_name ?? NO_NAME}・${formatShortDateTime(leave.created_at)}`,
    },
  ];
  return (
    <div className="flex flex-col gap-2 border-t border-border pt-2">
      <p className="text-sm font-bold">もとになったコーチのお休み</p>
      <DescriptionList items={items} label="もとになったコーチのお休み" />
    </div>
  );
}

/** キャンセルの経緯。値ごとに文にし、記録のない値は「記録がありません」と書く（推測で埋めない）。 */
function CancelDetails({ change, slotStartsAt }: { change: BookingStatusChange; slotStartsAt: string }) {
  const hours = hoursBeforeStart(change, slotStartsAt);
  const reason = reasonLabel(change);
  const items: DescriptionItem[] = [
    { term: "キャンセルした人", value: cancellerLabel(change) },
    { term: "開始まで", value: hoursBeforeStartLabel(hours) },
    { term: "ビデオ通話の枠", value: quotaLabel(change) },
    ...(reason === undefined ? [] : [{ term: "理由", value: reason }]),
  ];
  return (
    <>
      <DescriptionList items={items} label="キャンセルの経緯" />
      {isLegacyCancel(change) && <Notice message={hours === undefined ? LEGACY_CANCEL_NO_HOURS : LEGACY_CANCEL} status="info" />}
      {isCoachLeaveCancel(change) &&
        (change.coach_leave ? (
          <CoachLeaveRecord leave={change.coach_leave} />
        ) : (
          <p className="text-sm text-text-muted">{LEAVE_MISSING}</p>
        ))}
    </>
  );
}

/** 状態が変わった記録 1 件のカード（押せない）。頭に状態のタグと日時。 */
function StatusChangeCard({ change, slotStartsAt }: { change: BookingStatusChange; slotStartsAt: string }) {
  const time = formatShortDateTime(change.changed_at);
  return (
    <article aria-label={`${time} ${BOOKING_STATUS_LABELS[change.status]}`}>
      <Card>
        <div className="flex items-center gap-4">
          <BookingStatusBadge status={change.status} />
          <time dateTime={change.changed_at} className="text-sm tabular-nums">
            {time}
          </time>
        </div>
        {change.status === "confirmed" && <p className="text-sm">予約が確定しました。</p>}
        {change.status === "cancelled" && <CancelDetails change={change} slotStartsAt={slotStartsAt} />}
      </Card>
    </article>
  );
}

/**
 * 本部の「予約の詳細」。予約 1 件の生徒・コーチ・メニュー・枠の時刻・種類・今の状態と、状態が変わった記録を
 * getBooking で一度だけ読む。状態の記録は契約の正本どおり返った順（時刻の古い順）のまま、1 件 1 枚のカードで出し、
 * キャンセルの記録には経緯（キャンセルした人・開始まで・ビデオ通話の枠・理由・もとになったコーチのお休み）を添える
 * （specs/ux/issue-1030.yaml）。
 * `bookingId` がない・API が 404 のときは「見つからない」として、ほかの失敗とは分けて出す。
 */
export function BookingDetail({
  api,
  bookingId,
}: {
  api: Pick<HeadquartersApi, "getBooking">;
  bookingId: string | undefined;
}) {
  const [state, setState] = useState<State>(bookingId ? { kind: "loading" } : { kind: "not_found" });

  useEffect(() => {
    if (!bookingId) return;
    let active = true;
    void api.getBooking(bookingId).then((response) => {
      if (!active) return;
      if (response.ok) setState({ kind: "loaded", detail: response.data });
      else if (response.code === "not_found") setState({ kind: "not_found" });
      else setState({ kind: "failed", message: response.message });
    });
    return () => {
      active = false;
    };
  }, [api, bookingId]);

  if (state.kind === "loading") return <SectionLoading message="予約の詳細を読み込んでいます…" />;
  if (state.kind === "not_found") {
    return <ResultMessage result={{ ok: false, message: NOT_FOUND }} />;
  }
  if (state.kind === "failed") return <ResultMessage result={{ ok: false, message: state.message }} />;

  const { booking, slot, status_changes, coach_display_name, student_display_name } = state.detail;
  return (
    <div className="space-y-6">
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="font-semibold">生徒</dt>
        <dd>{student_display_name ?? NO_NAME}</dd>
        <dt className="font-semibold">コーチ</dt>
        <dd>{coach_display_name ?? NO_NAME}</dd>
        <dt className="font-semibold">レッスンメニュー</dt>
        <dd>{bookingLessonMenuLabel(booking.lesson_menu)}</dd>
        <dt className="font-semibold">枠の時刻</dt>
        <dd>{slotTimeLabel(slot)}</dd>
        <dt className="font-semibold">種類</dt>
        <dd>{booking.is_trial ? "体験予約" : "—"}</dd>
        <dt className="font-semibold">今の状態</dt>
        <dd>
          <BookingStatusBadge status={booking.status} />
        </dd>
        <dt className="font-semibold">予約した日時</dt>
        <dd>{formatDateTime(booking.created_at)}</dd>
      </dl>

      <div>
        <h3 className="mb-2 font-semibold">状態が変わった記録</h3>
        {status_changes.length === 0 ? (
          <EmptyState message={NO_CHANGES} variant="section" />
        ) : (
          <div className="flex flex-col gap-2">
            {status_changes.map((change, index) => (
              <StatusChangeCard key={`${change.status}-${change.changed_at}-${index}`} change={change} slotStartsAt={slot.starts_at} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
