"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import {
  type CoachReviews,
  CoachReviewSummaryView,
  loadCoachReviews,
} from "@/components/CoachReviewSummaryView";
import { ResultMessage } from "@/components/ui/ResultMessage";

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

type ReviewsState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "loaded"; reviews: CoachReviews; maxRating: MaxRating | null }
  | { kind: "failed"; message: string };

/** 「この点数以下」の選択肢（5 以下はすべてと同じなので出さない）。URL のクエリの確かめにも使う。 */
export const MAX_RATINGS = [1, 2, 3, 4] as const;
export type MaxRating = (typeof MAX_RATINGS)[number];

/** coachId が空ならコーチを選んでいない、maxRating が null なら点を問わない（すべて）。 */
export type CoachReviewFilter = { coachId: string; maxRating: MaxRating | null };

// 選択肢の外の点（URL の書き換えなど）で返った 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = "点数の指定が正しくありません。点数を選び直してください。";

/**
 * 本部の「コーチの評価」。開いたときにコーチの一覧（listCoaches）を取り、表示名から選んだコーチの
 * 評価の件数と平均（getCoachReviewSummary）と一覧（listCoachReviews）を出す。
 * 一覧の並びは契約の正本どおり API が済ませているので、返った順のまま選択肢にする。
 * 「点数」で選んだ点以下の評価だけに一覧を絞る（API の `max_rating`。件数と平均は絞らない）。コーチと同じく選んだらすぐ読み直す。
 * `initialCoachId` がコーチの一覧にあれば、読み込んだあとにそのコーチを選んだときと同じ流れで評価を読む
 * （一覧にない ID なら「選んでください」のまま）。`initialMaxRating`（URL のクエリ）の点で開き、コーチか点数を選び直したら
 * `onFilterChange` で知らせる（URL に残すのは呼ぶ側）。
 */
export function CoachReviewPicker({
  api,
  initialCoachId,
  initialMaxRating = null,
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listCoaches" | "getCoachReviewSummary" | "listCoachReviews">;
  initialCoachId?: string;
  initialMaxRating?: MaxRating | null;
  onFilterChange?: (filter: CoachReviewFilter) => void;
}) {
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [filter, setFilter] = useState<CoachReviewFilter>({ coachId: "", maxRating: initialMaxRating });
  const [reviews, setReviews] = useState<ReviewsState>({ kind: "idle" });
  // 選び直したときに、前に選んだコーチの遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchCoaches = useCallback(async (): Promise<CoachesState> => {
    const response = await api.listCoaches();
    return response.ok
      ? { kind: "loaded", coaches: response.data.coaches }
      : { kind: "failed", message: response.message };
  }, [api]);

  const applyFilter = useCallback(
    async (next: CoachReviewFilter) => {
      setFilter(next);
      const request = ++latestRequest.current;
      if (next.coachId === "") {
        setReviews({ kind: "idle" });
        return;
      }
      setReviews({ kind: "loading" });
      const response = await loadCoachReviews(api, next.coachId, next.maxRating);
      if (request !== latestRequest.current) return;
      if (response.ok) {
        setReviews({ kind: "loaded", reviews: response.data, maxRating: next.maxRating });
        return;
      }
      setReviews({
        kind: "failed",
        message: response.code === "validation_failed" ? VALIDATION_FAILED : response.message,
      });
    },
    [api],
  );

  function changeFilter(next: CoachReviewFilter) {
    onFilterChange?.(next);
    void applyFilter(next);
  }

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchCoaches().then((next) => {
      if (!active) return;
      setCoaches(next);
      if (
        initialCoachId &&
        next.kind === "loaded" &&
        next.coaches.some((coach) => coach.coach_id === initialCoachId)
      ) {
        void applyFilter({ coachId: initialCoachId, maxRating: initialMaxRating });
      }
    });
    return () => {
      active = false;
    };
  }, [fetchCoaches, applyFilter, initialCoachId, initialMaxRating]);

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒がレッスンに付けた評価の、これまでのすべての件数と点の平均と、1 件ずつの点と一言です。点数を選ぶと、一覧をその点以下の評価だけにします（件数と平均はすべての評価のまま）。
      </p>
      {coaches.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {coaches.kind === "failed" && (
        <ResultMessage result={{ ok: false, message: coaches.message }} />
      )}
      {coaches.kind === "loaded" && coaches.coaches.length === 0 && (
        <p className="text-sm">コーチがまだいません</p>
      )}
      {coaches.kind === "loaded" && coaches.coaches.length > 0 && (
        <>
          <label className="block">
            コーチ
            <select
              className="mt-1 block w-full rounded-md border px-2 py-1"
              value={filter.coachId}
              onChange={(event) => changeFilter({ ...filter, coachId: event.target.value })}
            >
              <option value="">選んでください</option>
              {coaches.coaches.map((coach) => (
                <option key={coach.coach_id} value={coach.coach_id}>
                  {coach.display_name}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            点数
            <select
              className="mt-1 block rounded-md border px-2 py-1"
              value={filter.maxRating ?? ""}
              onChange={(event) =>
                changeFilter({ ...filter, maxRating: maxRatingOf(event.target.value) })
              }
            >
              <option value="">すべて</option>
              {MAX_RATINGS.map((rating) => (
                <option key={rating} value={rating}>
                  {rating} 点以下
                </option>
              ))}
            </select>
          </label>
        </>
      )}
      {reviews.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {reviews.kind === "failed" && (
        <ResultMessage result={{ ok: false, message: reviews.message }} />
      )}
      {reviews.kind === "loaded" && (
        <CoachReviewSummaryView
          summary={reviews.reviews.summary}
          reviews={reviews.reviews.reviews}
          maxRating={reviews.maxRating}
        />
      )}
    </div>
  );
}

/** 選択肢・URL のクエリの値を点にする。選択肢の点でなければ「すべて」（null）。 */
export function maxRatingOf(value: string | null): MaxRating | null {
  return MAX_RATINGS.find((rating) => String(rating) === value) ?? null;
}
