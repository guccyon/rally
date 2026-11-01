"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { StudentActivity } from "./StudentActivity";

/** URL で生徒を指すクエリの名前（`?student_id=`）。 */
export const STUDENT_ID_QUERY = "student_id";

/** その生徒の「利用の様子」の画面の URL。 */
export function studentActivityPath(studentId: string): string {
  return `/headquarters/students/activity?${new URLSearchParams({ [STUDENT_ID_QUERY]: studentId })}`;
}

/** 本部の画面: 生徒の一覧の行から開き、生徒 1 人の利用の様子（予約・受けたレッスン・キャンセルの件数）を見る。 */
export function StudentActivityHome({ token, studentId }: { token: string; studentId: string | undefined }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters/students" className="text-blue-600 underline">
          生徒の一覧へ戻る
        </Link>
      </p>
      <Section title="生徒の利用の様子">
        <StudentActivity api={api} studentId={studentId} />
      </Section>
    </>
  );
}
