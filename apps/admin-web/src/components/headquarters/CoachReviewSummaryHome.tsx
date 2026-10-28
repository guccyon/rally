"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { CoachReviewPicker } from "./CoachReviewPicker";

/** 本部の画面: コーチを一覧から選んで、そのコーチの評価の件数と平均と一覧を見る。 */
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
        <CoachReviewPicker api={api} />
      </Section>
    </>
  );
}
