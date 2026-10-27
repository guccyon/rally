"use client";

import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { coachApi } from "@/api/coach";
import { CoachReviewSummaryLookup } from "@/components/CoachReviewSummaryLookup";
import { Section } from "@/components/Section";

import { LiveSessionPanel } from "./LiveSessionPanel";
import { PublishSlotForm } from "./PublishSlotForm";
import { ReturnVideoReviewForm } from "./ReturnVideoReviewForm";

/** コーチの画面: 枠の公開・添削の返却・通話への参加（ADR 0002 Q3）と、自分の評価の件数と平均。 */
export function CoachHome({ token }: { token: string }) {
  const api = useMemo(() => coachApi(createApiClient(token)), [token]);
  return (
    <>
      <Section title="予約枠">
        <PublishSlotForm api={api} />
      </Section>
      <Section title="動画の添削">
        <ReturnVideoReviewForm api={api} />
      </Section>
      <Section title="通話への参加">
        <LiveSessionPanel api={api} />
      </Section>
      <Section title="自分の評価">
        <CoachReviewSummaryLookup
          api={api}
          description="自分のアカウント ID を入力すると、生徒がレッスンに付けた評価の、これまでのすべての件数と点の平均を出します（他のコーチの分は見られません）。"
        />
      </Section>
    </>
  );
}
