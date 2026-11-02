"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { BookingList } from "./BookingList";
import { BookingStudentFilter } from "./BookingStudentFilter";

/** 「予約」の画面の URL。 */
export const BOOKINGS_PATH = "/headquarters/bookings";

/** URL でコーチを絞った状態にするクエリの名前（`?coach_id=`）。 */
export const BOOKINGS_COACH_QUERY = "coach_id";

/** URL で生徒を絞った状態にするクエリの名前（`?student_id=`）。 */
export const BOOKINGS_STUDENT_QUERY = "student_id";

/** コーチ・生徒で絞った（どちらもなければ絞らない）「予約」の画面の URL。 */
function bookingsPath({ coachId, studentId }: { coachId?: string; studentId?: string }): string {
  const query = new URLSearchParams();
  if (coachId !== undefined) query.set(BOOKINGS_COACH_QUERY, coachId);
  if (studentId !== undefined) query.set(BOOKINGS_STUDENT_QUERY, studentId);
  const search = query.toString();
  return search === "" ? BOOKINGS_PATH : `${BOOKINGS_PATH}?${search}`;
}

/** そのコーチで絞った「予約」の画面の URL。 */
export function bookingListPath(coachId: string): string {
  return bookingsPath({ coachId });
}

/** その生徒で絞った「予約」の画面の URL。 */
export function studentBookingListPath(studentId: string): string {
  return bookingsPath({ studentId });
}

/**
 * 本部の画面: 予約の一覧（日付ごとに、どの予約が入っていて、確定待ちのまま残っていないか）を見る。
 * `coachId`（URL の `?coach_id=`）があれば、そのコーチの枠への予約だけに絞った状態で開く。
 * `studentId`（URL の `?student_id=`）があれば、その生徒の予約だけに絞った状態で開き、一覧の上にその生徒の名前を出す。
 * 絞り込みを外すリンクは、外す方の条件だけを除いた URL へ移る（もう片方の絞り込みは残る）。
 */
export function BookingListHome({
  token,
  coachId,
  studentId,
}: {
  token: string;
  coachId?: string;
  studentId?: string;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 space-x-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
        {coachId !== undefined && (
          <Link href={bookingsPath({ studentId })} className="text-primary underline">
            すべてのコーチの予約を見る
          </Link>
        )}
        {studentId !== undefined && (
          <Link href={bookingsPath({ coachId })} className="text-primary underline">
            すべての生徒の予約を見る
          </Link>
        )}
      </p>
      <Section title="予約">
        {studentId !== undefined && <BookingStudentFilter key={studentId} api={api} studentId={studentId} />}
        <BookingList api={api} coachId={coachId} studentId={studentId} />
      </Section>
    </>
  );
}
