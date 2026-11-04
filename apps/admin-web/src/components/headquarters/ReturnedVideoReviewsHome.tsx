"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo, useState } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { BookingStudentFilter } from "./BookingStudentFilter";
import { periodError } from "./bookingPeriod";
import { ReturnedVideoReviewList, type ReturnedVideoReviewFilter, SPORTS } from "./ReturnedVideoReviewList";

/** 「返した動画添削」の画面の URL。 */
export const RETURNED_VIDEO_REVIEWS_PATH = "/headquarters/returned-video-reviews";

/** URL に絞り込みを残すクエリの名前（契約の `listReturnedVideoReviews` のクエリと同じ）。 */
const FROM_QUERY = "from";
const TO_QUERY = "to";
const COACH_QUERY = "coach_id";
const SPORT_QUERY = "sport";
const LESSON_MENU_QUERY = "lesson_menu_id";
/** URL で生徒を絞った状態にするクエリの名前（`?student_id=`）。 */
export const RETURNED_VIDEO_REVIEWS_STUDENT_QUERY = "student_id";

const NO_FILTER: ReturnedVideoReviewFilter = { period: null, coachId: "", sport: "", studentId: "", lessonMenuId: "" };

/** 絞り込みを残した「返した動画添削」の URL。期間がない・「すべて」の条件はクエリに出さない。 */
export function returnedVideoReviewsPath(filter: ReturnedVideoReviewFilter): string {
  const query = new URLSearchParams();
  if (filter.period !== null) {
    query.set(FROM_QUERY, filter.period.from);
    query.set(TO_QUERY, filter.period.to);
  }
  if (filter.coachId !== "") query.set(COACH_QUERY, filter.coachId);
  if (filter.sport !== "") query.set(SPORT_QUERY, filter.sport);
  if (filter.studentId !== "") query.set(RETURNED_VIDEO_REVIEWS_STUDENT_QUERY, filter.studentId);
  if (filter.lessonMenuId !== "") query.set(LESSON_MENU_QUERY, filter.lessonMenuId);
  const search = query.toString();
  return search === "" ? RETURNED_VIDEO_REVIEWS_PATH : `${RETURNED_VIDEO_REVIEWS_PATH}?${search}`;
}

/** その生徒で絞った（期間は API の既定の）「返した動画添削」の URL。 */
export function studentReturnedVideoReviewsPath(studentId: string): string {
  return returnedVideoReviewsPath({ ...NO_FILTER, studentId });
}

/**
 * URL のクエリから絞り込みを読む。期間は開始日と終了日がそろって正しいときだけ使い、そうでなければ API の既定に任せる。
 * スポーツは契約の値でなければ「すべて」として扱う。コーチ・生徒・レッスンメニューはそのまま API に渡す（UUID でなければ API の
 * 422 の理由を一覧が出す）。
 */
export function returnedVideoReviewFilterFrom(params: Pick<URLSearchParams, "get">): ReturnedVideoReviewFilter {
  const period = { from: params.get(FROM_QUERY) ?? "", to: params.get(TO_QUERY) ?? "" };
  const sport = params.get(SPORT_QUERY);
  return {
    period: periodError(period) === null ? period : null,
    coachId: params.get(COACH_QUERY) ?? "",
    sport: SPORTS.find((value) => value === sport) ?? "",
    studentId: params.get(RETURNED_VIDEO_REVIEWS_STUDENT_QUERY) ?? "",
    lessonMenuId: params.get(LESSON_MENU_QUERY) ?? "",
  };
}

/**
 * 本部の画面: コーチが返した動画添削を、期間・コーチ・スポーツ・レッスンメニューで絞って、送ってから返すまでにかかった時間とともに見る。
 * 絞り込みは URL のクエリ（`?from=&to=&coach_id=&sport=&student_id=&lesson_menu_id=`）に残し、再読み込み・共有で同じ一覧を開く。
 * 生徒（`?student_id=`）で絞っていれば、予約の画面と同じく一覧の上にその生徒の名前を出し、「すべての生徒の記録を見る」で
 * 生徒の絞り込みだけを外す（期間・コーチ・スポーツ・レッスンメニューは最後に「この条件で見る」で読んだ条件のまま残る）。
 */
export function ReturnedVideoReviewsHome({
  token,
  initialFilter,
}: {
  token: string;
  initialFilter?: ReturnedVideoReviewFilter;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 生徒の絞り込みを外すリンクの行き先に、いま見ている期間・コーチ・スポーツ・レッスンメニューを残すための、最後に URL に残した条件。
  const [applied, setApplied] = useState<ReturnedVideoReviewFilter>(initialFilter ?? NO_FILTER);
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: ReturnedVideoReviewFilter) => {
      setApplied(filter);
      router.replace(returnedVideoReviewsPath(filter), { scroll: false });
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
        <Link href="/headquarters/pending-video-reviews" className="text-primary underline">
          返却待ちの動画添削を見る
        </Link>
        {studentId !== "" && (
          <Link href={returnedVideoReviewsPath({ ...applied, studentId: "" })} className="text-primary underline">
            すべての生徒の記録を見る
          </Link>
        )}
      </p>
      <Section title="返した動画添削">
        {studentId !== "" && <BookingStudentFilter key={studentId} api={api} studentId={studentId} />}
        <ReturnedVideoReviewList api={api} initialFilter={initialFilter} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
