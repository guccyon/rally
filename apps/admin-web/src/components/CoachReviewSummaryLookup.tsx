"use client";

import { type FormEvent, useState } from "react";

import {
  type CoachReviews,
  type CoachReviewsApi,
  CoachReviewSummaryView,
  loadCoachReviews,
} from "@/components/CoachReviewSummaryView";
import { ResultMessage } from "@/components/ResultMessage";

/**
 * コーチ ID を入力して、そのコーチの評価の件数と平均（getCoachReviewSummary）を見る（本部の画面）。
 * `listCoachReviews` を渡すと、同じ ID で評価の一覧（listCoachReviews）も取り、件数と平均の下に出す。
 * どちらかが失敗したら、どちらも出さずに失敗の文言だけを出す。
 * コーチの「自分の評価」は入力せずに本人の分を出す（`coach/MyCoachReviews`）。
 */
export function CoachReviewSummaryLookup({
  api,
  description,
}: {
  api: CoachReviewsApi;
  description: string;
}) {
  const [coachId, setCoachId] = useState("");
  const [result, setResult] = useState<CoachReviews | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setResult(null);
    if (coachId.trim() === "") {
      setError("コーチ ID を入力してください。");
      return;
    }
    setError(null);
    setBusy(true);
    const response = await loadCoachReviews(api, coachId.trim());
    setBusy(false);
    if (response.ok) {
      setResult(response.data);
    } else {
      setError(response.message);
    }
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">{description}</p>
      <form onSubmit={onSubmit} className="space-y-3">
        <label className="block">
          コーチ ID
          <input
            className="mt-1 block w-full rounded border px-2 py-1 font-mono"
            value={coachId}
            onChange={(event) => setCoachId(event.target.value)}
          />
        </label>
        <button
          type="submit"
          disabled={busy}
          className="rounded bg-blue-600 px-4 py-1 text-white disabled:opacity-50"
        >
          評価を見る
        </button>
      </form>
      {error !== null && <ResultMessage result={{ ok: false, message: error }} />}
      {result !== null && <CoachReviewSummaryView summary={result.summary} reviews={result.reviews} />}
    </div>
  );
}
