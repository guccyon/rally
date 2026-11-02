"use client";

import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { bookingLessonMenuLabel } from "@/components/coach/bookingLessonMenu";
import { formatDateTime, formatTime, toJstDate } from "@/components/labels";

import { BookingStatusBadge } from "./BookingStatusBadge";

type HeadquartersBookingDetail = Schemas["HeadquartersBookingDetail"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; detail: HeadquartersBookingDetail }
  | { kind: "not_found" }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";
const NOT_FOUND = "この予約は見つかりません。予約の一覧から選び直してください。";

/** 枠の時刻。終わりが開始と同じ日（日本時間）なら終わりは時刻だけにする。 */
function slotTimeLabel(slot: Schemas["Slot"]): string {
  const end = toJstDate(slot.starts_at) === toJstDate(slot.ends_at) ? formatTime(slot.ends_at) : formatDateTime(slot.ends_at);
  return `${formatDateTime(slot.starts_at)} 〜 ${end}`;
}

/**
 * 本部の「予約の詳細」。予約 1 件の生徒・コーチ・メニュー・枠の時刻・種類・今の状態と、状態が変わった記録を
 * getBooking で一度だけ読む。状態の記録は契約の正本どおり返った順（時刻の古い順）のまま出す。
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

  if (state.kind === "loading") return <p className="text-sm text-gray-500">読み込み中…</p>;
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
          <p className="text-sm">状態が変わった記録はありません</p>
        ) : (
          <table aria-label="状態が変わった記録" className="w-full text-left text-sm">
            <thead>
              <tr>
                <th className="py-1">日時</th>
                <th className="py-1">変わった後の状態</th>
              </tr>
            </thead>
            <tbody>
              {status_changes.map((change, index) => (
                <tr key={`${change.status}-${change.changed_at}-${index}`} className="border-t">
                  <td className="py-1">{formatDateTime(change.changed_at)}</td>
                  <td className="py-1">
                    <BookingStatusBadge status={change.status} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
