"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { PendingVideoReviewList } from "./PendingVideoReviewList";

/** 「返却待ちの動画添削」の画面の URL。 */
export const PENDING_VIDEO_REVIEWS_PATH = "/headquarters/pending-video-reviews";

/** 本部の画面: コーチがまだ返していない動画添削を、コーチを問わず送った時刻の古い順に見る。 */
export function PendingVideoReviewsHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 flex gap-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
        <Link href="/headquarters/returned-video-reviews" className="text-primary underline">
          返した動画添削の記録を見る
        </Link>
      </p>
      <Section title="返却待ちの動画添削">
        <PendingVideoReviewList api={api} />
      </Section>
    </>
  );
}
