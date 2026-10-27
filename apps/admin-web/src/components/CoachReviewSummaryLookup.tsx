"use client";

import { type FormEvent, useState } from "react";

import type { Schemas } from "@/api/client";
import type { Result } from "@/api/call";
import { ResultMessage } from "@/components/ResultMessage";

type GetCoachReviewSummary = (coachId: string) => Promise<Result<Schemas["CoachReviewSummary"]>>;

/** 点の平均の表示。丸めは契約（getCoachReviewSummary）の正本どおり API が済ませているので、桁を揃えるだけ。 */
export function formatAverageRating(averageRating: number | null): string {
  return averageRating === null ? "評価はまだありません" : averageRating.toFixed(2);
}

/**
 * コーチの評価の件数と平均（getCoachReviewSummary）。コーチと本部で共通。
 * トークンは不透明なもの（中身を読まない）なので、コーチも自分のアカウント ID を入力する。
 */
export function CoachReviewSummaryLookup({
  api,
  description,
}: {
  api: { getCoachReviewSummary: GetCoachReviewSummary };
  description: string;
}) {
  const [coachId, setCoachId] = useState("");
  const [summary, setSummary] = useState<Schemas["CoachReviewSummary"] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setSummary(null);
    if (coachId.trim() === "") {
      setError("コーチ ID を入力してください。");
      return;
    }
    setError(null);
    setBusy(true);
    const response = await api.getCoachReviewSummary(coachId.trim());
    setBusy(false);
    if (response.ok) {
      setSummary(response.data);
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
      {summary !== null && (
        <dl aria-label="評価の件数と平均" className="grid grid-cols-[auto_1fr] gap-x-4 text-sm">
          <dt>コーチ ID</dt>
          <dd className="font-mono">{summary.coach_id}</dd>
          <dt>評価の件数</dt>
          <dd>{summary.review_count} 件</dd>
          <dt>点の平均（1〜5）</dt>
          <dd>{formatAverageRating(summary.average_rating)}</dd>
        </dl>
      )}
    </div>
  );
}
