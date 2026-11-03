"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { BookingStudentFilter } from "./BookingStudentFilter";
import { LowRatedReviewList } from "./LowRatedReviewList";

/** 「低い評価」の画面の URL。 */
export const LOW_RATED_REVIEWS_PATH = "/headquarters/low-reviews";

/** URL で生徒を絞った状態にするクエリの名前（契約の `listLowRatedLessonReviews` の `student_id` と同じ）。 */
export const LOW_RATED_REVIEWS_STUDENT_QUERY = "student_id";

/** その生徒で絞った「低い評価」の URL。 */
export function studentLowRatedReviewsPath(studentId: string): string {
  const query = new URLSearchParams({ [LOW_RATED_REVIEWS_STUDENT_QUERY]: studentId });
  return `${LOW_RATED_REVIEWS_PATH}?${query.toString()}`;
}

/**
 * 本部の画面: 生徒が付けた点の低いレッスンの評価を、コーチを問わず新しい順に見る。
 * 生徒（`?student_id=`）で絞っていれば、返事のないメッセージと同じく一覧の上にその生徒の名前を出し、
 * 「すべての生徒の低い評価を見る」で生徒の絞り込みを外す。
 */
export function LowRatedReviewsHome({ token, studentId = "" }: { token: string; studentId?: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 flex flex-wrap gap-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
        {studentId !== "" && (
          <Link href={LOW_RATED_REVIEWS_PATH} className="text-primary underline">
            すべての生徒の低い評価を見る
          </Link>
        )}
      </p>
      <Section title="低い評価">
        {studentId !== "" && <BookingStudentFilter key={studentId} api={api} studentId={studentId} />}
        <LowRatedReviewList key={studentId} api={api} studentId={studentId === "" ? undefined : studentId} />
      </Section>
    </>
  );
}
