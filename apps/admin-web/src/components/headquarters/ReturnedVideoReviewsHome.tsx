"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { ReturnedVideoReviewList } from "./ReturnedVideoReviewList";

/** 本部の画面: コーチが返した動画添削を、期間とコーチで絞って、送ってから返すまでにかかった時間とともに見る。 */
export function ReturnedVideoReviewsHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 flex gap-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
        <Link href="/headquarters/pending-video-reviews" className="text-blue-600 underline">
          返却待ちの動画添削を見る
        </Link>
      </p>
      <Section title="返した動画添削">
        <ReturnedVideoReviewList api={api} />
      </Section>
    </>
  );
}
