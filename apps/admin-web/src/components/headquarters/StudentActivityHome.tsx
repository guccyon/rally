"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { studentBookingListPath } from "./BookingListHome";
import { studentLiveSessionRecordsPath } from "./LiveSessionRecordsHome";
import { studentOverdueLiveSessionsPath } from "./OverdueLiveSessionsHome";
import { studentReturnedVideoReviewsPath } from "./ReturnedVideoReviewsHome";
import { StudentActivity } from "./StudentActivity";
import { StudentAnnouncements } from "./StudentAnnouncements";

/** URL で生徒を指すクエリの名前（`?student_id=`）。 */
export const STUDENT_ID_QUERY = "student_id";

/** その生徒の「利用の様子」の画面の URL。 */
export function studentActivityPath(studentId: string): string {
  return `/headquarters/students/activity?${new URLSearchParams({ [STUDENT_ID_QUERY]: studentId })}`;
}

/**
 * 本部の画面: 生徒の一覧の行から開き、生徒 1 人の利用の様子（予約・受けたレッスン・キャンセルの件数・動画の送付数と返却待ち）と
 * 受け取ったお知らせを見る。欄ごとに別に読むので、片方が読めなくてももう片方は出る。
 * 生徒の ID がなければ受け取ったお知らせの欄と、予約・ライブレッスンの記録・終わっていない通話・返した動画添削の記録へのリンクは
 * 出さない（利用の様子の欄が「見つかりません」を出す）。
 * 「この生徒の予約を見る」「この生徒のライブレッスンの記録を見る」「この生徒の終わっていない通話を見る」
 * 「この生徒の返した動画添削の記録を見る」は、それぞれの画面を
 * その生徒で絞った状態で開く。リンクは生徒の ID だけで決まるので、様子の読み込みを待たない。
 */
export function StudentActivityHome({ token, studentId }: { token: string; studentId: string | undefined }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 space-x-4 text-sm">
        <Link href="/headquarters/students" className="text-primary underline">
          生徒の一覧へ戻る
        </Link>
        {studentId !== undefined && (
          <Link href={studentBookingListPath(studentId)} className="text-primary underline">
            この生徒の予約を見る
          </Link>
        )}
        {studentId !== undefined && (
          <Link href={studentLiveSessionRecordsPath(studentId)} className="text-primary underline">
            この生徒のライブレッスンの記録を見る
          </Link>
        )}
        {studentId !== undefined && (
          <Link href={studentOverdueLiveSessionsPath(studentId)} className="text-primary underline">
            この生徒の終わっていない通話を見る
          </Link>
        )}
        {studentId !== undefined && (
          <Link href={studentReturnedVideoReviewsPath(studentId)} className="text-primary underline">
            この生徒の返した動画添削の記録を見る
          </Link>
        )}
      </p>
      <Section title="生徒の利用の様子">
        <StudentActivity api={api} studentId={studentId} />
      </Section>
      {studentId !== undefined && (
        <Section title="受け取ったお知らせ">
          <StudentAnnouncements api={api} studentId={studentId} />
        </Section>
      )}
    </>
  );
}
