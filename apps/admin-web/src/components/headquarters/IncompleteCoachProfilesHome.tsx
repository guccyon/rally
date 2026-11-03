"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { ACCEPTANCE_STATUSES, SPORTS } from "./CoachAcceptanceList";
import { type IncompleteCoachProfileFilter, IncompleteCoachProfileList } from "./IncompleteCoachProfileList";

/** 「自己紹介の足りないコーチ」の画面の URL。 */
export const INCOMPLETE_COACH_PROFILES_PATH = "/headquarters/coaches/incomplete-profiles";

/** URL に絞り込みを残すクエリの名前（契約の `listIncompleteCoachProfiles` のクエリと同じ）。 */
const SPORT_QUERY = "sport";
const STATUS_QUERY = "acceptance_status";
const NAME_QUERY = "q";

/** 絞り込みを残した「自己紹介の足りないコーチ」の URL。「すべて」の条件はクエリに出さない。 */
export function incompleteCoachProfilesPath(filter: IncompleteCoachProfileFilter): string {
  const query = new URLSearchParams();
  if (filter.sport !== "") query.set(SPORT_QUERY, filter.sport);
  if (filter.acceptanceStatus !== "") query.set(STATUS_QUERY, filter.acceptanceStatus);
  if (filter.q !== "") query.set(NAME_QUERY, filter.q);
  const search = query.toString();
  return search === "" ? INCOMPLETE_COACH_PROFILES_PATH : `${INCOMPLETE_COACH_PROFILES_PATH}?${search}`;
}

/**
 * URL のクエリから絞り込みを読む。契約の値でなければ「すべて」として扱う。
 * 検索語は前後の空白を除いてそのまま使う（長すぎる語はサーバーの 422 の理由を一覧に出す）。
 */
export function incompleteCoachProfileFilterFrom(params: Pick<URLSearchParams, "get">): IncompleteCoachProfileFilter {
  const sport = params.get(SPORT_QUERY);
  const status = params.get(STATUS_QUERY);
  return {
    sport: SPORTS.find((value) => value === sport) ?? "",
    acceptanceStatus: ACCEPTANCE_STATUSES.find((value) => value === status) ?? "",
    q: params.get(NAME_QUERY)?.trim() ?? "",
  };
}

/**
 * 本部の画面: プロフィール（自己紹介）に空の項目があるコーチを見る。
 * 絞り込みは URL のクエリ（`?sport=&acceptance_status=&q=`）に残し、再読み込み・共有で同じ一覧を開く。
 */
export function IncompleteCoachProfilesHome({
  token,
  initialFilter,
}: {
  token: string;
  initialFilter?: IncompleteCoachProfileFilter;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (filter: IncompleteCoachProfileFilter) => router.replace(incompleteCoachProfilesPath(filter), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="自己紹介の足りないコーチ">
        <IncompleteCoachProfileList api={api} initialFilter={initialFilter} onFilterChange={keepInUrl} />
      </Section>
    </>
  );
}
