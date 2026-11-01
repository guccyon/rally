"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { ACCEPTANCE_STATUSES, type CoachAcceptanceFilter, CoachAcceptanceList, SPORTS } from "./CoachAcceptanceList";

/** 「コーチの受付」の画面の URL。 */
export const COACH_ACCEPTANCE_PATH = "/headquarters/coaches";

/** URL に絞り込みを残すクエリの名前（契約の `listCoaches` のクエリと同じ）。 */
const STATUS_QUERY = "acceptance_status";
const SPORT_QUERY = "sport";

/** 絞り込みを残した「コーチの受付」の URL。「すべて」の条件はクエリに出さない。 */
export function coachAcceptancePath(filter: CoachAcceptanceFilter): string {
  const query = new URLSearchParams();
  if (filter.acceptanceStatus !== "") query.set(STATUS_QUERY, filter.acceptanceStatus);
  if (filter.sport !== "") query.set(SPORT_QUERY, filter.sport);
  const search = query.toString();
  return search === "" ? COACH_ACCEPTANCE_PATH : `${COACH_ACCEPTANCE_PATH}?${search}`;
}

/** URL のクエリから絞り込みを読む。契約の値でなければ「すべて」として扱う。 */
export function coachAcceptanceFilterFrom(params: Pick<URLSearchParams, "get">): CoachAcceptanceFilter {
  const status = params.get(STATUS_QUERY);
  const sport = params.get(SPORT_QUERY);
  return {
    acceptanceStatus: ACCEPTANCE_STATUSES.find((value) => value === status) ?? "",
    sport: SPORTS.find((value) => value === sport) ?? "",
  };
}

/**
 * 本部の画面: コーチの一覧で受付の状態を見て、受付を止める・再開する。
 * 絞り込みは URL のクエリ（`?acceptance_status=&sport=`）に残し、再読み込み・共有で同じ一覧を開く。
 */
export function CoachAcceptanceHome({
  token,
  initialFilter,
}: {
  token: string;
  initialFilter?: CoachAcceptanceFilter;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: CoachAcceptanceFilter) => router.replace(coachAcceptancePath(filter), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="コーチの受付">
        <CoachAcceptanceList api={api} initialFilter={initialFilter} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
