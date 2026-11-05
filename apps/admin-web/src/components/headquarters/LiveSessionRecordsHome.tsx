"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { BookingStudentFilter } from "./BookingStudentFilter";
import { LiveSessionRecordList } from "./LiveSessionRecordList";

/** 「ビデオ通話の記録」の画面の URL。 */
export const LIVE_SESSION_RECORDS_PATH = "/headquarters/live-sessions";

/** URL で生徒を絞った状態にするクエリの名前（`?student_id=`）。 */
export const LIVE_SESSION_RECORDS_STUDENT_QUERY = "student_id";

/** その生徒で絞った「ビデオ通話の記録」の画面の URL。 */
export function studentLiveSessionRecordsPath(studentId: string): string {
  return `${LIVE_SESSION_RECORDS_PATH}?${new URLSearchParams({ [LIVE_SESSION_RECORDS_STUDENT_QUERY]: studentId })}`;
}

/**
 * 本部の画面: 問い合わせを受けたときに、ビデオ通話（通話セッション）がいつ始まりいつ終わったかを期間・コーチ・スポーツ・レッスンメニューで引く。
 * `studentId`（URL の `?student_id=`）があれば、その生徒の記録だけに絞った状態で開き、一覧の上にその生徒の名前を出す。
 * 「すべての生徒の記録を見る」で生徒の絞り込みを外す（予約の画面の生徒の絞り込みと同じ形）。
 */
export function LiveSessionRecordsHome({ token, studentId }: { token: string; studentId?: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 space-x-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
        {studentId !== undefined && (
          <Link href={LIVE_SESSION_RECORDS_PATH} className="text-primary underline">
            すべての生徒の記録を見る
          </Link>
        )}
      </p>
      <Section title="ビデオ通話の記録">
        {studentId !== undefined && <BookingStudentFilter key={studentId} api={api} studentId={studentId} />}
        <LiveSessionRecordList key={studentId} api={api} studentId={studentId} />
      </Section>
    </>
  );
}
