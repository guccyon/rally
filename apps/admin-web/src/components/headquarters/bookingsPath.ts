// 本部の「予約」の画面の URL（絞り込みと期間をクエリで持つ）。予約の一覧と予約の詳細（戻り先）が同じ組み立てを使う。

import type { Period } from "./bookingPeriod";

/** 「予約」の画面の URL。 */
export const BOOKINGS_PATH = "/headquarters/bookings";

/** URL でコーチを絞った状態にするクエリの名前（`?coach_id=`）。 */
export const BOOKINGS_COACH_QUERY = "coach_id";

/** URL で生徒を絞った状態にするクエリの名前（`?student_id=`）。 */
export const BOOKINGS_STUDENT_QUERY = "student_id";

/** URL でレッスンメニューを絞った状態にするクエリの名前（`?lesson_menu_id=`。契約の `listBookings` のクエリと同じ）。 */
export const BOOKINGS_LESSON_MENU_QUERY = "lesson_menu_id";

/** URL で期間の始まりを指すクエリの名前（`?from=`。契約の `listBookings` のクエリと同じ）。 */
export const BOOKINGS_FROM_QUERY = "from";

/** URL で期間の終わりを指すクエリの名前（`?to=`）。 */
export const BOOKINGS_TO_QUERY = "to";

/** 一覧を絞る条件（ないものは絞らない）と、開く期間（なければ既定の期間）。 */
export type BookingsQuery = { coachId?: string; studentId?: string; lessonMenuId?: string; period?: Period };

/** コーチ・生徒・レッスンメニューで絞った（どれもなければ絞らない）「予約」の画面の URL。 */
export function bookingsPath({ coachId, studentId, lessonMenuId, period }: BookingsQuery): string {
  const query = new URLSearchParams();
  if (coachId !== undefined) query.set(BOOKINGS_COACH_QUERY, coachId);
  if (studentId !== undefined) query.set(BOOKINGS_STUDENT_QUERY, studentId);
  if (lessonMenuId !== undefined) query.set(BOOKINGS_LESSON_MENU_QUERY, lessonMenuId);
  if (period !== undefined) {
    query.set(BOOKINGS_FROM_QUERY, period.from);
    query.set(BOOKINGS_TO_QUERY, period.to);
  }
  const search = query.toString();
  return search === "" ? BOOKINGS_PATH : `${BOOKINGS_PATH}?${search}`;
}

