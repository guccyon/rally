"use client";

import { useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import {
  type CoachReviews,
  CoachReviewSummaryView,
  loadCoachReviews,
} from "@/components/CoachReviewSummaryView";
import { ResultMessage } from "@/components/ResultMessage";
import { LessonReviewReplyForm } from "@/components/coach/LessonReviewReplyForm";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; reviews: CoachReviews }
  | { kind: "failed"; message: string };

const NOT_COACH = "コーチのアカウントでログインしたときだけ、自分の評価を見られます。";

/**
 * コーチの「自分の評価」。開いたときに、ログイン中の本人のアカウント（getMyAccount）の ID で
 * 評価の件数と平均（getCoachReviewSummary）と一覧（listCoachReviews）を出す。一覧の各行には、評価したレッスンの
 * メニュー（`lesson_menu`）があれば「メニュー: 〇〇」も出し、その下で評価に返信する（replyToLessonReview。LessonReviewReplyForm）。
 * 保存した返信は、一覧を取り直さずに応答の返信をその行に入れる。
 * トークンは不透明なもの（中身を読まない）なので、本人の ID は API に聞く。
 */
export function MyCoachReviews({
  api,
}: {
  api: Pick<
    CoachApi,
    "getMyAccount" | "getCoachReviewSummary" | "listCoachReviews" | "replyToLessonReview"
  >;
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

  function onReplySaved(reviewId: string, reply: Schemas["LessonReviewReply"]) {
    setState((current) =>
      current.kind === "loaded"
        ? {
            kind: "loaded",
            reviews: {
              ...current.reviews,
              reviews: current.reviews.reviews.map((review) =>
                review.review_id === reviewId ? { ...review, reply } : review,
              ),
            },
          }
        : current,
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒がレッスンに付けたあなたへの評価の、これまでのすべての件数と点の平均と、1 件ずつの点と一言です。各評価に返信を 1 つ（200 文字まで）付けられ、書き直すと置き換わります。
      </p>
      {state.kind === "loading" && <p className="text-sm text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <CoachReviewSummaryView
          summary={state.reviews.summary}
          reviews={state.reviews.reviews}
          showLessonMenu
          renderReply={(review) => (
            <LessonReviewReplyForm
              api={api}
              review={review}
              onSaved={(reply) => onReplySaved(review.review_id, reply)}
            />
          )}
        />
      )}
    </div>
  );
}
