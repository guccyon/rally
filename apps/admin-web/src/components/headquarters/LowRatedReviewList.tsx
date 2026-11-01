"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

import { coachReviewSummaryPath } from "./CoachReviewSummaryHome";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["LowRatedLessonReviewList"] }
  | { kind: "failed"; message: string };

/**
 * 本部の「低い評価」。開いたときに低い評価の一覧（listLowRatedLessonReviews）を一度だけ読み、1 件ずつ点・コーチ・
 * メニュー（あれば）・一言・送った時刻を出す。しきい値・期間・並べ方は契約の正本どおり API が済ませているので、
 * 返った順のまま出し、しきい値と期間は応答の値を添える。コーチの名前からそのコーチの評価の画面へ移れる。
 */
export function LowRatedReviewList({
  api,
}: {
  api: Pick<HeadquartersApi, "listLowRatedLessonReviews">;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let active = true;
    void api.listLowRatedLessonReviews().then((response) => {
      if (!active) return;
      setState(
        response.ok
          ? { kind: "loaded", list: response.data }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api]);

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒がレッスンに付けた評価のうち、点の低いものを送った時刻の新しい順に見られます。コーチの名前から、そのコーチの評価へ移れます。
      </p>
      {state.kind === "loading" && <p className="text-sm text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <p className="text-sm text-muted">
          直近 {state.list.period_days} 日に送られた、{state.list.max_rating} 点以下の評価です。
        </p>
      )}
      {state.kind === "loaded" && state.list.reviews.length === 0 && (
        <p className="text-sm">低い評価はまだありません</p>
      )}
      {state.kind === "loaded" && state.list.reviews.length > 0 && (
        <ul aria-label="低い評価の一覧" className="divide-y rounded border text-sm">
          {state.list.reviews.map((review) => (
            <li key={review.review_id} className="space-y-1 px-3 py-2">
              <p>
                <span className="font-semibold">{review.rating} 点</span>
                <span className="text-muted">（5 点満点）</span>
              </p>
              <p>
                コーチ:{" "}
                <Link
                  href={coachReviewSummaryPath(review.coach_id)}
                  className="text-primary underline"
                >
                  {review.coach_display_name ?? "（名前を表示できません）"}
                </Link>
              </p>
              {review.lesson_menu && <p>メニュー: {review.lesson_menu.name}</p>}
              <p>{review.comment}</p>
              <p className="text-muted">{formatDateTime(review.submitted_at)}</p>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
