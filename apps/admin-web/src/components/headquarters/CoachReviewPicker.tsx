"use client";

import { type ChangeEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import {
  type CoachReviews,
  CoachReviewSummaryView,
  loadCoachReviews,
} from "@/components/CoachReviewSummaryView";
import { ResultMessage } from "@/components/ResultMessage";

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

type ReviewsState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "loaded"; reviews: CoachReviews }
  | { kind: "failed"; message: string };

/**
 * 本部の「コーチの評価」。開いたときにコーチの一覧（listCoaches）を取り、表示名から選んだコーチの
 * 評価の件数と平均（getCoachReviewSummary）と一覧（listCoachReviews）を出す。
 * 一覧の並びは契約の正本どおり API が済ませているので、返った順のまま選択肢にする。
 */
export function CoachReviewPicker({
  api,
}: {
  api: Pick<HeadquartersApi, "listCoaches" | "getCoachReviewSummary" | "listCoachReviews">;
}) {
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [coachId, setCoachId] = useState("");
  const [reviews, setReviews] = useState<ReviewsState>({ kind: "idle" });
  // 選び直したときに、前に選んだコーチの遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchCoaches = useCallback(async (): Promise<CoachesState> => {
    const response = await api.listCoaches();
    return response.ok
      ? { kind: "loaded", coaches: response.data.coaches }
      : { kind: "failed", message: response.message };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchCoaches().then((next) => {
      if (active) setCoaches(next);
    });
    return () => {
      active = false;
    };
  }, [fetchCoaches]);

  async function onChange(event: ChangeEvent<HTMLSelectElement>) {
    const selected = event.target.value;
    setCoachId(selected);
    const request = ++latestRequest.current;
    if (selected === "") {
      setReviews({ kind: "idle" });
      return;
    }
    setReviews({ kind: "loading" });
    const response = await loadCoachReviews(api, selected);
    if (request !== latestRequest.current) return;
    setReviews(
      response.ok
        ? { kind: "loaded", reviews: response.data }
        : { kind: "failed", message: response.message },
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒がレッスンに付けた評価の、これまでのすべての件数と点の平均と、1 件ずつの点と一言です。
      </p>
      {coaches.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {coaches.kind === "failed" && (
        <ResultMessage result={{ ok: false, message: coaches.message }} />
      )}
      {coaches.kind === "loaded" && coaches.coaches.length === 0 && (
        <p className="text-sm">コーチがまだいません</p>
      )}
      {coaches.kind === "loaded" && coaches.coaches.length > 0 && (
        <label className="block">
          コーチ
          <select
            className="mt-1 block w-full rounded border px-2 py-1"
            value={coachId}
            onChange={(event) => void onChange(event)}
          >
            <option value="">選んでください</option>
            {coaches.coaches.map((coach) => (
              <option key={coach.coach_id} value={coach.coach_id}>
                {coach.display_name}
              </option>
            ))}
          </select>
        </label>
      )}
      {reviews.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {reviews.kind === "failed" && (
        <ResultMessage result={{ ok: false, message: reviews.message }} />
      )}
      {reviews.kind === "loaded" && (
        <CoachReviewSummaryView summary={reviews.reviews.summary} reviews={reviews.reviews.reviews} />
      )}
    </div>
  );
}
