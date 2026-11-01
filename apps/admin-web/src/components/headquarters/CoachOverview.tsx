"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { formatAverageRating } from "@/components/CoachReviewSummaryView";
import { ResultMessage } from "@/components/ResultMessage";
import { LessonStatsTable } from "@/components/coach/LessonStats";

import { CoachAcceptanceBadge } from "./CoachAcceptanceBadge";
import { coachReviewSummaryPath } from "./CoachReviewSummaryHome";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; overview: Schemas["CoachOverview"] }
  | { kind: "not_found" }
  | { kind: "failed"; message: string };

/**
 * 本部の「コーチの様子」。開いたときにコーチ 1 人の様子（getCoachOverview）を一度だけ読み、受付の状態・評価の件数と平均
 * （「コーチの評価」へのリンク）・今月と先月の実績・公開中のレッスンメニューの数を出す。数え方は契約が正本で、画面は返った値を
 * 出すだけ。`coachId` がない・404 `not_found` なら「見つかりません」、それ以外の失敗はその文言を出す。受付は変えない。
 */
export function CoachOverview({
  api,
  coachId,
}: {
  api: Pick<HeadquartersApi, "getCoachOverview">;
  coachId: string | undefined;
}) {
  const [state, setState] = useState<State>(
    coachId === undefined ? { kind: "not_found" } : { kind: "loading" },
  );

  useEffect(() => {
    if (coachId === undefined) return;
    let active = true;
    void api.getCoachOverview(coachId).then((response) => {
      if (!active) return;
      if (response.ok) setState({ kind: "loaded", overview: response.data });
      else if (response.code === "not_found") setState({ kind: "not_found" });
      else setState({ kind: "failed", message: response.message });
    });
    return () => {
      active = false;
    };
  }, [api, coachId]);

  if (state.kind === "loading") return <p className="text-sm text-muted">読み込み中…</p>;
  if (state.kind === "failed") return <ResultMessage result={{ ok: false, message: state.message }} />;
  if (state.kind === "not_found") {
    return <p className="text-sm">このコーチは見つかりません。コーチの一覧から選び直してください。</p>;
  }

  const { overview } = state;
  return (
    <div className="space-y-4">
      <h3 className="text-base font-semibold">{overview.display_name}</h3>
      <dl aria-label="コーチの様子" className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        <dt>受付</dt>
        <dd>
          <CoachAcceptanceBadge status={overview.acceptance_status} />
        </dd>
        <dt>評価の件数</dt>
        <dd>{overview.review_count} 件</dd>
        <dt>点の平均（1〜5）</dt>
        <dd>
          {formatAverageRating(overview.average_rating)}
          <Link href={coachReviewSummaryPath(overview.coach_id)} className="ml-3 text-primary underline">
            コーチの評価を見る
          </Link>
        </dd>
        <dt>公開中のレッスンメニュー</dt>
        <dd>{overview.published_lesson_menu_count} 件</dd>
      </dl>
      <div className="space-y-2">
        <h3 className="font-semibold">今月と先月の実績</h3>
        <LessonStatsTable stats={overview} />
      </div>
    </div>
  );
}
