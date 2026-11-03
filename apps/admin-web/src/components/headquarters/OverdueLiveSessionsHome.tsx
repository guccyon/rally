"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { OverdueLiveSessionList, type OverdueLiveSessionFilter } from "./OverdueLiveSessionList";
import { SPORTS } from "./PendingVideoReviewList";

/** 「終わっていない通話」の画面の URL。 */
export const OVERDUE_LIVE_SESSIONS_PATH = "/headquarters/overdue-live-sessions";

/** URL に絞り込みを残すクエリの名前（契約の `listOverdueLiveLessonSessions` のクエリと同じ `?sport=`・`?coach_id=`）。 */
const OVERDUE_LIVE_SESSIONS_SPORT_QUERY = "sport";
const OVERDUE_LIVE_SESSIONS_COACH_QUERY = "coach_id";

/** 絞り込みを残した「終わっていない通話」の画面の URL。「すべて」（空）の条件はクエリに出さない。 */
export function overdueLiveSessionsPath(filter: OverdueLiveSessionFilter): string {
  const query = new URLSearchParams();
  if (filter.sport !== "") query.set(OVERDUE_LIVE_SESSIONS_SPORT_QUERY, filter.sport);
  if (filter.coachId !== "") query.set(OVERDUE_LIVE_SESSIONS_COACH_QUERY, filter.coachId);
  const search = query.toString();
  return search === "" ? OVERDUE_LIVE_SESSIONS_PATH : `${OVERDUE_LIVE_SESSIONS_PATH}?${search}`;
}

/**
 * URL のクエリから絞り込みを読む。スポーツは選択肢の値でなければ「すべて」として扱う。
 * コーチはそのまま API に渡す（UUID でなければ API の 422 の理由を一覧が出す）。
 */
export function overdueLiveSessionFilterFrom(params: Pick<URLSearchParams, "get">): OverdueLiveSessionFilter {
  const sport = params.get(OVERDUE_LIVE_SESSIONS_SPORT_QUERY);
  return {
    sport: SPORTS.find((value) => value === sport) ?? "",
    coachId: params.get(OVERDUE_LIVE_SESSIONS_COACH_QUERY) ?? "",
  };
}

/**
 * 本部の画面: 始まったまま終了の記録がなく、時間の過ぎた通話セッションを見る。スポーツ・コーチの絞り込みは URL のクエリ
 * （`?sport=&coach_id=`）に残し、再読み込み・共有で同じ条件の通話を開く。
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
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: OverdueLiveSessionFilter) => router.replace(overdueLiveSessionsPath(filter), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="終わっていない通話">
        <OverdueLiveSessionList api={api} initialFilter={initialFilter} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
