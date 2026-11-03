"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { StudentDirectory } from "./StudentDirectory";

/** 「生徒」の画面の URL。 */
export const STUDENT_DIRECTORY_PATH = "/headquarters/students";

/** URL でコーチを絞った状態にするクエリの名前（契約の `listStudents` のクエリと同じ `?coach_id=`）。 */
export const STUDENT_DIRECTORY_COACH_QUERY = "coach_id";

/** コーチで絞った「生徒」の画面の URL。「すべて」（空）はクエリに出さない。 */
export function studentDirectoryPath(coachId: string): string {
  return coachId === ""
    ? STUDENT_DIRECTORY_PATH
    : `${STUDENT_DIRECTORY_PATH}?${new URLSearchParams({ [STUDENT_DIRECTORY_COACH_QUERY]: coachId })}`;
}

/**
 * 本部の画面: 生徒の一覧（どのスポーツの生徒が、どの契約で、最近予約しているか、どのコーチを予約したか）を見る。
 * コーチの絞り込みは URL のクエリ（`?coach_id=`）に残し、再読み込み・共有で同じコーチの生徒を開く。
 */
export function StudentDirectoryHome({ token, initialCoachId }: { token: string; initialCoachId?: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (coachId: string) => router.replace(studentDirectoryPath(coachId), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="生徒">
        <StudentDirectory api={api} initialCoachId={initialCoachId} onCoachChange={keepInUrl} />
      </Section>
    </>
  );
}
