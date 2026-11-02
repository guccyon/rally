"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { SPORTS } from "./CoachAcceptanceList";
import { type IncompleteCoachProfileFilter, IncompleteCoachProfileList } from "./IncompleteCoachProfileList";

/** 「自己紹介の足りないコーチ」の画面の URL。 */
export const INCOMPLETE_COACH_PROFILES_PATH = "/headquarters/coaches/incomplete-profiles";

/** URL に絞り込みを残すクエリの名前（契約の `listIncompleteCoachProfiles` のクエリと同じ）。 */
const SPORT_QUERY = "sport";

/** 絞り込みを残した「自己紹介の足りないコーチ」の URL。「すべて」の条件はクエリに出さない。 */
export function incompleteCoachProfilesPath(filter: IncompleteCoachProfileFilter): string {
  return filter.sport === ""
    ? INCOMPLETE_COACH_PROFILES_PATH
    : `${INCOMPLETE_COACH_PROFILES_PATH}?${new URLSearchParams({ [SPORT_QUERY]: filter.sport })}`;
}

/** URL のクエリから絞り込みを読む。契約の値でなければ「すべて」として扱う。 */
export function incompleteCoachProfileFilterFrom(params: Pick<URLSearchParams, "get">): IncompleteCoachProfileFilter {
  const sport = params.get(SPORT_QUERY);
  return { sport: SPORTS.find((value) => value === sport) ?? "" };
}

/**
 * 本部の画面: プロフィール（自己紹介）に空の項目があるコーチを見る。
 * 絞り込みは URL のクエリ（`?sport=`）に残し、再読み込み・共有で同じ一覧を開く。
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
