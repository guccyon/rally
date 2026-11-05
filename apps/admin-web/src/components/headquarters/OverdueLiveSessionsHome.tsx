"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo, useState } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { BookingStudentFilter } from "./BookingStudentFilter";
import { OverdueLiveSessionList, type OverdueLiveSessionFilter } from "./OverdueLiveSessionList";
import { SPORTS } from "./PendingVideoReviewList";

/** 「終わっていないビデオ通話」の画面の URL。 */
export const OVERDUE_LIVE_SESSIONS_PATH = "/headquarters/overdue-live-sessions";

/**
 * URL に絞り込みを残すクエリの名前（契約の `listOverdueLiveLessonSessions` のクエリと同じ `?sport=`・`?coach_id=`・
 * `?student_id=`）。
 */
const OVERDUE_LIVE_SESSIONS_SPORT_QUERY = "sport";
const OVERDUE_LIVE_SESSIONS_COACH_QUERY = "coach_id";
export const OVERDUE_LIVE_SESSIONS_STUDENT_QUERY = "student_id";

const NO_FILTER: OverdueLiveSessionFilter = { sport: "", coachId: "", studentId: "" };

/** 絞り込みを残した「終わっていないビデオ通話」の画面の URL。「すべて」（空）の条件はクエリに出さない。 */
export function overdueLiveSessionsPath(filter: OverdueLiveSessionFilter): string {
  const query = new URLSearchParams();
  if (filter.sport !== "") query.set(OVERDUE_LIVE_SESSIONS_SPORT_QUERY, filter.sport);
  if (filter.coachId !== "") query.set(OVERDUE_LIVE_SESSIONS_COACH_QUERY, filter.coachId);
  if (filter.studentId !== "") query.set(OVERDUE_LIVE_SESSIONS_STUDENT_QUERY, filter.studentId);
  const search = query.toString();
  return search === "" ? OVERDUE_LIVE_SESSIONS_PATH : `${OVERDUE_LIVE_SESSIONS_PATH}?${search}`;
}

/** その生徒で絞った「終わっていないビデオ通話」の画面の URL。 */
export function studentOverdueLiveSessionsPath(studentId: string): string {
  return overdueLiveSessionsPath({ ...NO_FILTER, studentId });
}

/**
 * URL のクエリから絞り込みを読む。スポーツは選択肢の値でなければ「すべて」として扱う。
 * コーチと生徒はそのまま API に渡す（UUID でなければ API の 422 の理由を一覧が出す）。
 */
export function overdueLiveSessionFilterFrom(params: Pick<URLSearchParams, "get">): OverdueLiveSessionFilter {
  const sport = params.get(OVERDUE_LIVE_SESSIONS_SPORT_QUERY);
  return {
    sport: SPORTS.find((value) => value === sport) ?? "",
    coachId: params.get(OVERDUE_LIVE_SESSIONS_COACH_QUERY) ?? "",
    studentId: params.get(OVERDUE_LIVE_SESSIONS_STUDENT_QUERY) ?? "",
  };
}

/**
 * 本部の画面: 始まったまま終了の記録がなく、時間の過ぎた通話セッションを見る。スポーツ・コーチ・生徒の絞り込みは URL のクエリ
 * （`?sport=&coach_id=&student_id=`）に残し、再読み込み・共有で同じ条件のビデオ通話を開く。
 * 生徒（`?student_id=`）で絞っていれば、ビデオ通話の記録と同じく一覧の上にその生徒の名前を出し、
 * 「すべての生徒の終わっていないビデオ通話を見る」で生徒の絞り込みだけを外す（スポーツ・コーチはいまの条件のまま残る）。
 */
export function OverdueLiveSessionsHome({
  token,
  initialFilter,
}: {
  token: string;
  initialFilter?: OverdueLiveSessionFilter;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 生徒の絞り込みを外すリンクの行き先に、いまのスポーツ・コーチを残すための、最後に URL に残した条件。
  const [applied, setApplied] = useState<OverdueLiveSessionFilter>(initialFilter ?? NO_FILTER);
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: OverdueLiveSessionFilter) => {
      setApplied(filter);
      router.replace(overdueLiveSessionsPath(filter), { scroll: false });
    },
    [router],
  );
  const studentId = applied.studentId;
  return (
    <>
      <p className="mb-4 flex flex-wrap gap-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
        {studentId !== "" && (
          <Link href={overdueLiveSessionsPath({ ...applied, studentId: "" })} className="text-primary underline">
            すべての生徒の終わっていないビデオ通話を見る
          </Link>
        )}
      </p>
      <Section title="終わっていないビデオ通話">
        {studentId !== "" && <BookingStudentFilter key={studentId} api={api} studentId={studentId} />}
        <OverdueLiveSessionList api={api} initialFilter={initialFilter} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
