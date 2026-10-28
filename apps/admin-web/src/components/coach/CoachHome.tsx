"use client";

import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { coachApi } from "@/api/coach";
import { Section } from "@/components/Section";

import { CoachProfileForm } from "./CoachProfileForm";
import { LiveSessionPanel } from "./LiveSessionPanel";
import { MyCoachReviews } from "./MyCoachReviews";
import { PublishSlotForm } from "./PublishSlotForm";
import { ReturnVideoReviewForm } from "./ReturnVideoReviewForm";

/**
 * コーチの画面: 枠の公開・添削の返却・通話への参加（ADR 0002 Q3）と、開いたときから出す自分の評価の件数と平均と一覧と、
 * 自己紹介の編集。
 */
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
        <MyCoachReviews api={api} />
      </Section>
      <Section title="自己紹介">
        <CoachProfileForm api={api} />
      </Section>
    </>
  );
}
