"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

import { coachReviewSummaryPath } from "./CoachReviewSummaryHome";
import { LowRatedReviewHandlingAction } from "./LowRatedReviewHandlingAction";

type LowRatedReview = Schemas["LowRatedLessonReview"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["LowRatedLessonReviewList"] }
  | { kind: "failed"; message: string };

const NO_COACH_NAME = "（名前を表示できません）";

function HandlingStatus({ review }: { review: LowRatedReview }) {
  const handling = review.handling;
  if (handling === undefined || handling === null) return <p>対応: 未対応</p>;
  return <p>対応: 対応済み（{formatDateTime(handling.handled_at)}）</p>;
}

/**
 * 本部の「低い評価」。開いたときに低い評価の一覧（listLowRatedLessonReviews）を読み、1 件ずつ点・コーチ・
 * メニュー（あれば）・一言・送った時刻・対応の状況を出す。しきい値・期間・並べ方は契約の正本どおり API が済ませているので、
 * 返った順のまま出し、しきい値と期間は応答の値を添える。コーチの名前からそのコーチの評価の画面へ移れる。
 * 「未対応だけ」は API の `handled=false` に渡す。未対応の行は確かめてから対応済みにする（markLowRatedLessonReviewHandled）。
 */
export function LowRatedReviewList({
  api,
}: {
  api: Pick<HeadquartersApi, "listLowRatedLessonReviews" | "markLowRatedLessonReviewHandled">;
}) {
  const [unhandledOnly, setUnhandledOnly] = useState(false);
  const [state, setState] = useState<State>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  // 切り替えを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchReviews = useCallback(
    async (onlyUnhandled: boolean): Promise<State> => {
      const response = await api.listLowRatedLessonReviews(
        onlyUnhandled ? { handled: false } : {},
      );
      return response.ok
        ? { kind: "loaded", list: response.data }
        : { kind: "failed", message: response.message };
    },
    [api],
  );

  // 開いたときの読み込み（条件なし）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchReviews(false).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchReviews]);

  async function reload(onlyUnhandled: boolean) {
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchReviews(onlyUnhandled);
    if (request === latestRequest.current) setState(loaded);
  }

  async function changeUnhandledOnly(next: boolean) {
    setUnhandledOnly(next);
    setResult(null);
    await reload(next);
  }

  async function markHandled(review: LowRatedReview) {
    const response = await api.markLowRatedLessonReviewHandled(review.review_id);
    if (response.ok) {
      setResult({ ok: true, message: "評価を対応済みにしました。" });
      // 「未対応だけ」でも、押した行はすぐには消さずに対応済みと出す（次に読み直したときに一覧から外れる）。
      const handling = response.data;
      setState((current) =>
        current.kind === "loaded"
          ? {
              ...current,
              list: {
                ...current.list,
                reviews: current.list.reviews.map((row) =>
                  row.review_id === review.review_id ? { ...row, handling } : row,
                ),
              },
            }
          : current,
      );
      return;
    }
    // 409（低い評価でない）・404（評価がない）などは ErrorCode の文言を出し、一覧を今の状態に取り直す。
    setResult(response);
    if (response.code !== undefined) await reload(unhandledOnly);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒がレッスンに付けた評価のうち、点の低いものを送った時刻の新しい順に見られます。コーチの名前から、そのコーチの評価へ移れます。
      </p>
      <label className="flex items-center gap-1 text-sm">
        <input
          type="checkbox"
          checked={unhandledOnly}
          onChange={(event) => void changeUnhandledOnly(event.target.checked)}
        />
        未対応だけ
      </label>
      <ResultMessage result={result} />
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <p className="text-sm text-gray-500">
          直近 {state.list.period_days} 日に送られた、{state.list.max_rating} 点以下の評価です。
        </p>
      )}
      {state.kind === "loaded" && state.list.reviews.length === 0 && (
        <p className="text-sm">
          {unhandledOnly ? "未対応の低い評価はありません" : "低い評価はまだありません"}
        </p>
      )}
      {state.kind === "loaded" && state.list.reviews.length > 0 && (
        <ul aria-label="低い評価の一覧" className="divide-y rounded border text-sm">
          {state.list.reviews.map((review) => {
            const coachName = review.coach_display_name ?? NO_COACH_NAME;
            const handled = review.handling !== undefined && review.handling !== null;
            return (
              <li key={review.review_id} className="space-y-1 px-3 py-2">
                <p>
                  <span className="font-semibold">{review.rating} 点</span>
                  <span className="text-gray-500">（5 点満点）</span>
                </p>
                <p>
                  コーチ:{" "}
                  <Link
                    href={coachReviewSummaryPath(review.coach_id)}
                    className="text-blue-600 underline"
                  >
                    {coachName}
                  </Link>
                </p>
                {review.lesson_menu && <p>メニュー: {review.lesson_menu.name}</p>}
                <p>{review.comment}</p>
                <p className="text-gray-500">{formatDateTime(review.submitted_at)}</p>
                <HandlingStatus review={review} />
                {!handled && (
                  <LowRatedReviewHandlingAction
                    label={`${coachName}への${formatDateTime(review.submitted_at)}の評価`}
                    onConfirm={() => markHandled(review)}
                  />
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
