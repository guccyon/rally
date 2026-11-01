"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { CoachOverview } from "./CoachOverview";

/** コーチを指すクエリの名前（`?coach_id=`）。 */
export const COACH_OVERVIEW_QUERY = "coach_id";

/** そのコーチの様子の画面の URL。 */
export function coachOverviewPath(coachId: string): string {
  return `/headquarters/coach-overview?${new URLSearchParams({ [COACH_OVERVIEW_QUERY]: coachId })}`;
}

/** 本部の画面: コーチの受付の一覧の行から開く、コーチ 1 人の様子。 */
export function CoachOverviewHome({ token, coachId }: { token: string; coachId: string | undefined }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 space-x-4 text-sm">
        <Link href="/headquarters/coaches" className="text-primary underline">
          コーチの一覧へ戻る
        </Link>
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="コーチの様子">
        <CoachOverview api={api} coachId={coachId} />
      </Section>
    </>
  );
}
