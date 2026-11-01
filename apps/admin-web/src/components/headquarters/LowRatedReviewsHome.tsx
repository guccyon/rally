"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { LowRatedReviewList } from "./LowRatedReviewList";

/** 「低い評価」の画面の URL。 */
export const LOW_RATED_REVIEWS_PATH = "/headquarters/low-reviews";

/** 本部の画面: 生徒が付けた点の低いレッスンの評価を、コーチを問わず新しい順に見る。 */
export function LowRatedReviewsHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="低い評価">
        <LowRatedReviewList api={api} />
      </Section>
    </>
  );
}
