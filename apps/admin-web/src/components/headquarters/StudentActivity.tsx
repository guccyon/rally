"use client";

import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { bookingLessonMenuLabel } from "@/components/coach/bookingLessonMenu";
import { BOOKING_STATUS_LABELS, LESSON_TYPE_LABELS, formatDateTime } from "@/components/labels";

type StudentActivityData = Schemas["StudentActivity"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; activity: StudentActivityData }
  | { kind: "not_found" }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";
const NOT_FOUND = "この生徒は見つかりません。生徒の一覧から選び直してください。";

/**
 * 本部の「生徒の利用の様子」。生徒 1 人の表示名・これからの予約・直近の受けたレッスン・キャンセルの件数を
 * getStudentActivity で一度だけ読む。並びと件数（直近のレッスンは最大 20 件）は契約の正本どおり返った順のまま出す。
 * `studentId` がない・API が 404 のときは「見つからない」として、ほかの失敗とは分けて出す。
 */
export function StudentActivity({
  api,
  studentId,
}: {
  api: Pick<HeadquartersApi, "getStudentActivity">;
  studentId: string | undefined;
}) {
  const [state, setState] = useState<State>(studentId ? { kind: "loading" } : { kind: "not_found" });

  useEffect(() => {
    if (!studentId) return;
    let active = true;
    void api.getStudentActivity(studentId).then((response) => {
      if (!active) return;
      if (response.ok) setState({ kind: "loaded", activity: response.data });
      else if (response.code === "not_found") setState({ kind: "not_found" });
      else setState({ kind: "failed", message: response.message });
    });
    return () => {
      active = false;
    };
  }, [api, studentId]);

  if (state.kind === "loading") return <p className="text-sm text-text-muted">読み込み中…</p>;
  if (state.kind === "not_found") {
    return <ResultMessage result={{ ok: false, message: NOT_FOUND }} />;
  }
  if (state.kind === "failed") return <ResultMessage result={{ ok: false, message: state.message }} />;

  const { activity } = state;
  return (
    <div className="space-y-6">
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="font-semibold">表示名</dt>
        <dd>{activity.display_name}</dd>
        <dt className="font-semibold">キャンセルした予約</dt>
        <dd>{activity.cancelled_booking_count} 件（これまでのすべて）</dd>
      </dl>

      <div>
        <h3 className="mb-2 font-semibold">これからの予約</h3>
        {activity.upcoming_bookings.length === 0 ? (
          <p className="text-sm">これからの予約はありません</p>
        ) : (
          <table aria-label="これからの予約" className="w-full text-left text-sm">
            <thead>
              <tr>
                <th className="py-1">開始</th>
                <th className="py-1">状態</th>
                <th className="py-1">コーチ</th>
                <th className="py-1">レッスンメニュー</th>
                <th className="py-1">種類</th>
              </tr>
            </thead>
            <tbody>
              {activity.upcoming_bookings.map(({ booking, coach_display_name }) => (
                <tr key={booking.booking_id} className="border-t">
                  <td className="py-1">{formatDateTime(booking.starts_at)}</td>
                  <td className="py-1">{BOOKING_STATUS_LABELS[booking.status]}</td>
                  <td className="py-1">{coach_display_name ?? NO_NAME}</td>
                  <td className="py-1">{bookingLessonMenuLabel(booking.lesson_menu)}</td>
                  <td className="py-1">{booking.is_trial ? "体験予約" : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <div>
        <h3 className="mb-2 font-semibold">直近の受けたレッスン</h3>
        {activity.recent_lessons.length === 0 ? (
          <p className="text-sm">まだ受けたレッスンはありません</p>
        ) : (
          <>
            <p className="mb-2 text-sm text-text-muted">完了した新しい順に、最大 20 件まで出します。</p>
            <table aria-label="直近の受けたレッスン" className="w-full text-left text-sm">
              <thead>
                <tr>
                  <th className="py-1">完了</th>
                  <th className="py-1">種類</th>
                  <th className="py-1">コーチ</th>
                </tr>
              </thead>
              <tbody>
                {activity.recent_lessons.map((lesson) => (
                  <tr key={lesson.lesson_id} className="border-t">
                    <td className="py-1">{formatDateTime(lesson.completed_at)}</td>
                    <td className="py-1">{LESSON_TYPE_LABELS[lesson.lesson_type]}</td>
                    <td className="py-1">{lesson.coach_display_name ?? NO_NAME}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </div>
    </div>
  );
}
