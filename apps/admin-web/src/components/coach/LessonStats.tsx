"use client";

import { useEffect, useState } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { formatAverageRating } from "@/components/CoachReviewSummaryView";
import { ResultMessage } from "@/components/ResultMessage";
import { formatMonth } from "@/components/labels";

type MonthlyLessonStats = Schemas["MonthlyLessonStats"];

/** 表の行（見出しと、1 か月分の値の表示）。並びは通話・動画添削・評価の件数・評価の平均。 */
const ROWS: { label: string; value: (month: MonthlyLessonStats) => string }[] = [
  { label: "通話（同期レッスン）", value: (month) => `${month.live_lesson_count} 回` },
  { label: "動画添削（非同期レッスン）", value: (month) => `${month.async_lesson_count} 回` },
  { label: "評価の件数", value: (month) => `${month.review_count} 件` },
  { label: "点の平均（1〜5）", value: (month) => formatAverageRating(month.average_rating) },
];

function isEmptyMonth(month: MonthlyLessonStats): boolean {
  return month.live_lesson_count === 0 && month.async_lesson_count === 0 && month.review_count === 0;
}

/**
 * コーチのホーム（対応待ち）の上に出す、今月と先月のレッスンの実績（getMyLessonStats）。
 * 完了したレッスンの数を通話と動画添削に分けて、その月に受けた評価の件数と平均とともに月ごとの列で並べる。
 * 数え方（暦月の区切り・平均の丸め）は契約が正本で、画面は返った値を出すだけ。金額は出さない。
 */
export function LessonStats({ api }: { api: Pick<CoachApi, "getMyLessonStats"> }) {
  const [result, setResult] = useState<Result<Schemas["CoachLessonStats"]> | null>(null);

  useEffect(() => {
    let active = true;
    void api.getMyLessonStats().then((next) => {
      if (active) setResult(next);
    });
    return () => {
      active = false;
    };
  }, [api]);

  return (
    <div className="space-y-2">
      <h3 className="font-semibold">今月と先月の実績</h3>
      {!result && <p className="text-sm text-text-muted">読み込み中…</p>}
      {result && !result.ok && <ResultMessage result={{ ok: false, message: result.message }} />}
      {result?.ok && <LessonStatsTable stats={result.data} />}
    </div>
  );
}

/**
 * 今月と先月の実績の表。コーチのホーム（getMyLessonStats）と本部のコーチの様子（getCoachOverview。同じ形の月の項目）で共通。
 */
export function LessonStatsTable({
  stats,
}: {
  stats: Pick<Schemas["CoachLessonStats"], "current_month" | "previous_month">;
}) {
  const months = [
    { title: "今月", month: stats.current_month },
    { title: "先月", month: stats.previous_month },
  ];
  return (
    <>
      <table aria-label="今月と先月の実績" className="text-sm">
        <thead>
          <tr>
            <td />
            {months.map(({ title, month }) => (
              <th key={title} scope="col" className="px-3 text-right">
                {title}（{formatMonth(month.month)}）
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {ROWS.map((row) => (
            <tr key={row.label}>
              <th scope="row" className="pr-3 text-left font-normal">
                {row.label}
              </th>
              {months.map(({ title, month }) => (
                <td key={title} className="px-3 text-right">
                  {row.value(month)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {months.every(({ month }) => isEmptyMonth(month)) && (
        <p className="text-sm">今月と先月は、完了したレッスンも受けた評価もまだありません。</p>
      )}
      <p className="text-xs text-text-muted">
        レッスンは完了を記録した月、評価は生徒が送った月で数えます（日本時間）。
      </p>
    </>
  );
}
