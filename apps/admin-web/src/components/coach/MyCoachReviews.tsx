"use client";

import { useCallback, useEffect, useState } from "react";

import type { CoachApi } from "@/api/coach";
import {
  type CoachReviews,
  CoachReviewSummaryView,
  loadCoachReviews,
} from "@/components/CoachReviewSummaryView";
import { ResultMessage } from "@/components/ResultMessage";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; reviews: CoachReviews }
  | { kind: "failed"; message: string };

const NOT_COACH = "コーチのアカウントでログインしたときだけ、自分の評価を見られます。";

/**
 * コーチの「自分の評価」。開いたときに、ログイン中の本人のアカウント（getMyAccount）の ID で
 * 評価の件数と平均（getCoachReviewSummary）と一覧（listCoachReviews）を出す。
 * トークンは不透明なもの（中身を読まない）なので、本人の ID は API に聞く。
 */
export function MyCoachReviews({
  api,
}: {
  api: Pick<CoachApi, "getMyAccount" | "getCoachReviewSummary" | "listCoachReviews">;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  const fetchReviews = useCallback(async (): Promise<State> => {
    const account = await api.getMyAccount();
    if (!account.ok) return { kind: "failed", message: account.message };
    if (account.data.role !== "coach") return { kind: "failed", message: NOT_COACH };
    const response = await loadCoachReviews(api, account.data.account_id);
    return response.ok
      ? { kind: "loaded", reviews: response.data }
      : { kind: "failed", message: response.message };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchReviews().then((next) => {
      if (active) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchReviews]);

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒がレッスンに付けたあなたへの評価の、これまでのすべての件数と点の平均と、1 件ずつの点と一言です。
      </p>
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <CoachReviewSummaryView summary={state.reviews.summary} reviews={state.reviews.reviews} />
      )}
    </div>
  );
}
