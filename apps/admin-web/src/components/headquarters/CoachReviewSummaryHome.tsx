"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { CoachReviewSummaryLookup } from "@/components/CoachReviewSummaryLookup";
import { NotImplemented, Section } from "@/components/Section";

/** 本部の画面: コーチ ID を指定して、そのコーチの評価の件数と平均を見る。 */
export function CoachReviewSummaryHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="コーチの評価">
        <div className="space-y-4">
          <NotImplemented>
            コーチの一覧を取る API が admin-web の契約にないため、コーチ ID を入力して見ます。
          </NotImplemented>
          <CoachReviewSummaryLookup
            api={api}
            description="生徒がレッスンに付けた評価の、これまでのすべての件数と点の平均です。"
          />
        </div>
      </Section>
    </>
  );
}
