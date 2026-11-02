"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { SPORT_LABELS, formatDateTime, formatDay } from "@/components/labels";

import { MAX_PERIOD_DAYS, type Period, periodError } from "./bookingPeriod";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["ReturnedVideoReviewList"]; coachId: string }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

/** 期間が空なら API の既定（今日までの 7 日）に任せる。coachId が空ならすべてのコーチ。 */
type Filter = { period: Period | null; coachId: string };

const NO_NAME = "（名前を表示できません）";

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間かコーチの指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。`;

/**
 * 本部の「返した動画添削」。コーチが返した動画添削を、返却の日付（日本時間）の期間とコーチで絞って出す
 * （listReturnedVideoReviews）。開いたときは期間を付けずに読み、API の既定（今日までの 7 日）で実際に使った期間を入力欄に入れる。
 * かかった時間の数え方と並び（返却の新しい順）は契約の正本どおり API が済ませているので、返った順と値のまま出す。
 */
export function ReturnedVideoReviewList({
  api,
}: {
  api: Pick<HeadquartersApi, "listReturnedVideoReviews" | "listCoaches">;
}) {
  const [draft, setDraft] = useState<Filter>({ period: null, coachId: "" });
  const [draftError, setDraftError] = useState<string | null>(null);
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [state, setState] = useState<State>({ kind: "loading" });
  // 条件を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchList = useCallback(
    async (filter: Filter): Promise<State> => {
      const response = await api.listReturnedVideoReviews({
        ...(filter.period ?? {}),
        ...(filter.coachId === "" ? {} : { coach_id: filter.coachId }),
      });
      if (response.ok) return { kind: "loaded", list: response.data, coachId: filter.coachId };
      return {
        kind: "failed",
        message: response.code === "validation_failed" ? VALIDATION_FAILED : response.message,
      };
    },
    [api],
  );

  // 開いたときの読み込み（API の既定の期間・すべてのコーチ）。読めたら実際に使った期間を入力欄に入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchList({ period: null, coachId: "" }).then((next) => {
      if (!active || request !== latestRequest.current) return;
      setState(next);
      if (next.kind === "loaded") {
        const used = { from: next.list.from, to: next.list.to };
        setDraft((current) => (current.period === null ? { ...current, period: used } : current));
      }
    });
    return () => {
      active = false;
    };
  }, [fetchList]);

  // コーチの選択肢。読めなくても記録の一覧は出し、コーチの絞り込みだけを使えなくする。
  useEffect(() => {
    let active = true;
    void api.listCoaches().then((response) => {
      if (!active) return;
      setCoaches(
        response.ok
          ? { kind: "loaded", coaches: response.data.coaches }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const period = draft.period ?? { from: "", to: "" };
    const error = periodError(period);
    setDraftError(error);
    if (error !== null) return;
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchList({ period, coachId: draft.coachId });
    if (request === latestRequest.current) setState(result);
  }

  const period = draft.period ?? { from: "", to: "" };
  const coachOptions = coaches.kind === "loaded" ? coaches.coaches : [];

  return (
    <div className="space-y-4">
      <p className="text-sm">
        コーチが返した動画添削を、返した日付（日本時間）の新しい順に、送ってから返すまでにかかった時間とともに見られます。期間は{" "}
        {MAX_PERIOD_DAYS} 日まで選べます。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submit(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded border px-2 py-1"
            value={period.from}
            onChange={(event) => setDraft({ ...draft, period: { ...period, from: event.target.value } })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded border px-2 py-1"
            value={period.to}
            onChange={(event) => setDraft({ ...draft, period: { ...period, to: event.target.value } })}
          />
        </label>
        <label className="block">
          コーチ
          <select
            className="mt-1 block rounded border px-2 py-1"
            value={draft.coachId}
            disabled={coaches.kind !== "loaded"}
            onChange={(event) => setDraft({ ...draft, coachId: event.target.value })}
          >
            <option value="">すべて</option>
            {coachOptions.map((coach) => (
              <option key={coach.coach_id} value={coach.coach_id}>
                {coach.display_name}
              </option>
            ))}
          </select>
        </label>
        <button type="submit" className="rounded bg-blue-600 px-4 py-1 text-white">
          この条件で見る
        </button>
      </form>
      {coaches.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `コーチの一覧を読めませんでした。${coaches.message}` }}
        />
      )}
      {draftError !== null && <ResultMessage result={{ ok: false, message: draftError }} />}
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && <LoadedReviews list={state.list} filtered={state.coachId !== ""} />}
    </div>
  );
}

function LoadedReviews({
  list,
  filtered,
}: {
  list: Schemas["ReturnedVideoReviewList"];
  filtered: boolean;
}) {
  return (
    <div className="space-y-2">
      <p className="text-sm">
        {formatDay(list.from)} 〜 {formatDay(list.to)}・{list.reviews.length} 件
      </p>
      <p className="text-sm text-gray-500">
        かかった時間は送った時刻から返した時刻までです（1 時間に満たない端数は切り捨て）。
      </p>
      {list.reviews.length === 0 && (
        <p className="text-sm">
          {filtered
            ? "この期間にこのコーチが返した動画添削はありません"
            : "この期間に返した動画添削はありません"}
        </p>
      )}
      {list.reviews.length > 0 && (
        <table aria-label="返した動画添削の一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">生徒</th>
              <th className="py-1">コーチ</th>
              <th className="py-1">スポーツ</th>
              <th className="py-1">送った日時</th>
              <th className="py-1">返した日時</th>
              <th className="py-1">かかった時間</th>
            </tr>
          </thead>
          <tbody>
            {list.reviews.map((review) => (
              <tr key={review.video_id} className="border-t">
                <td className="py-1">{review.student_display_name ?? NO_NAME}</td>
                <td className="py-1">{review.coach_display_name ?? NO_NAME}</td>
                <td className="py-1">{SPORT_LABELS[review.sport]}</td>
                <td className="py-1">{formatDateTime(review.submitted_at)}</td>
                <td className="py-1">{formatDateTime(review.returned_at)}</td>
                <td className="py-1">{review.lead_time_hours} 時間</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
