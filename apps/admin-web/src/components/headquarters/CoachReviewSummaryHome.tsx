"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { CoachReviewPicker } from "./CoachReviewPicker";

/** URL でコーチを選んだ状態にするクエリの名前（`?coach_id=`）。 */
export const COACH_ID_QUERY = "coach_id";

/** そのコーチを選んだ状態で開く「コーチの評価」の画面の URL。 */
export function coachReviewSummaryPath(coachId: string): string {
  return `/headquarters/coach-review-summary?${new URLSearchParams({ [COACH_ID_QUERY]: coachId })}`;
}

/**
 * 本部の画面: コーチを一覧から選んで、そのコーチの評価の件数と平均と一覧を見る。
 * `initialCoachId`（URL の `?coach_id=`）があれば、そのコーチを選んだ状態で開く。
 */
export function CoachReviewSummaryHome({
  token,
  initialCoachId,
}: {
  token: string;
  initialCoachId?: string;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="コーチの評価">
        <CoachReviewPicker api={api} initialCoachId={initialCoachId} />
      </Section>
    </>
  );
}
