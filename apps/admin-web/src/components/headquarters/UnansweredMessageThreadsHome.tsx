"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo, useState } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { BookingStudentFilter } from "./BookingStudentFilter";
import {
  OLDER_THAN_HOURS_OPTIONS,
  UnansweredMessageThreadList,
  type UnansweredMessageThreadFilter,
} from "./UnansweredMessageThreadList";

/** 本部の「返事のないメッセージ」の画面の URL。 */
export const UNANSWERED_MESSAGE_THREADS_PATH = "/headquarters/unanswered-messages";

/** URL に絞り込みを残すクエリの名前（契約の `listUnansweredMessageThreads` のクエリと同じ）。 */
const OLDER_THAN_HOURS_QUERY = "older_than_hours";
const COACH_QUERY = "coach_id";
const STUDENT_QUERY = "student_id";

const NO_FILTER: UnansweredMessageThreadFilter = { olderThanHours: null, coachId: "", studentId: "" };

/** 絞り込みを残した「返事のないメッセージ」の URL。既定の時間・「すべて」の条件はクエリに出さない。 */
export function unansweredMessageThreadsPath(filter: UnansweredMessageThreadFilter): string {
  const query = new URLSearchParams();
  if (filter.olderThanHours !== null) query.set(OLDER_THAN_HOURS_QUERY, String(filter.olderThanHours));
  if (filter.coachId !== "") query.set(COACH_QUERY, filter.coachId);
  if (filter.studentId !== "") query.set(STUDENT_QUERY, filter.studentId);
  const search = query.toString();
  return search === "" ? UNANSWERED_MESSAGE_THREADS_PATH : `${UNANSWERED_MESSAGE_THREADS_PATH}?${search}`;
}

/** そのコーチで絞った「返事のないメッセージ」の URL。 */
export function coachUnansweredMessageThreadsPath(coachId: string): string {
  return unansweredMessageThreadsPath({ ...NO_FILTER, coachId });
}

/** その生徒で絞った「返事のないメッセージ」の URL。 */
export function studentUnansweredMessageThreadsPath(studentId: string): string {
  return unansweredMessageThreadsPath({ ...NO_FILTER, studentId });
}

/**
 * URL のクエリから絞り込みを読む。時間は選択肢の値でなければ既定（クエリなし）として扱う。
 * コーチと生徒はそのまま API に渡す（UUID でなければ API の 422 の理由を一覧が出す）。
 */
export function unansweredMessageThreadFilterFrom(params: Pick<URLSearchParams, "get">): UnansweredMessageThreadFilter {
  const olderThanHours = params.get(OLDER_THAN_HOURS_QUERY);
  const hours = OLDER_THAN_HOURS_OPTIONS.find((value) => String(value) === olderThanHours);
  return {
    olderThanHours: hours === undefined || hours === OLDER_THAN_HOURS_OPTIONS[0] ? null : hours,
    coachId: params.get(COACH_QUERY) ?? "",
    studentId: params.get(STUDENT_QUERY) ?? "",
  };
}

/**
 * 本部の画面: 生徒からのメッセージにコーチが返事をしていないやり取りを、待ち始めた時刻の古い順に、返事を待っている時間・コーチで絞って見る。
 * 絞り込みは URL のクエリ（`?older_than_hours=&coach_id=&student_id=`）に残し、再読み込み・共有で同じ一覧を開く。
 * 生徒（`?student_id=`）で絞っていれば、返却待ちの動画添削と同じく一覧の上にその生徒の名前を出し、
 * 「すべての生徒の返事のないメッセージを見る」で生徒の絞り込みだけを外す（時間・コーチはいまの条件のまま残る）。
 */
export function UnansweredMessageThreadsHome({
  token,
  initialFilter,
}: {
  token: string;
  initialFilter?: UnansweredMessageThreadFilter;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 生徒の絞り込みを外すリンクの行き先に、いまの時間・コーチを残すための、最後に URL に残した条件。
  const [applied, setApplied] = useState<UnansweredMessageThreadFilter>(initialFilter ?? NO_FILTER);
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: UnansweredMessageThreadFilter) => {
      setApplied(filter);
      router.replace(unansweredMessageThreadsPath(filter), { scroll: false });
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
          <Link href={unansweredMessageThreadsPath({ ...applied, studentId: "" })} className="text-primary underline">
            すべての生徒の返事のないメッセージを見る
          </Link>
        )}
      </p>
      <Section title="返事のないメッセージ">
        {studentId !== "" && <BookingStudentFilter key={studentId} api={api} studentId={studentId} />}
        <UnansweredMessageThreadList api={api} initialFilter={initialFilter} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
