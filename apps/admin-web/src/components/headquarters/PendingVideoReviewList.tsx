"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { SPORT_LABELS, formatDateTime } from "@/components/labels";

type Sport = Schemas["Sport"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["PendingVideoReviewList"] }
  | { kind: "failed"; message: string };

/** minElapsedHours が null なら経過時間で絞らない。 */
type Filter = { sport: Sport | ""; minElapsedHours: number | null };

const SPORTS = Object.keys(SPORT_LABELS) as Sport[];

/** 経過時間の下限の選択肢（時間）。 */
const MIN_ELAPSED_HOURS_OPTIONS = [24, 48, 72, 168] as const;

const NO_FILTER: Filter = { sport: "", minElapsedHours: null };

/** 送ってからの経過がこの時間以上の行を目立たせる（時間。API の `elapsed_hours` と比べる）。 */
export const OVERDUE_ELAPSED_HOURS = 48;

function isOverdue(video: { elapsed_hours: number }): boolean {
  return video.elapsed_hours >= OVERDUE_ELAPSED_HOURS;
}

/**
 * 本部の「返却待ちの動画添削」。コーチがまだ返していない動画添削の一覧（listPendingVideoReviews）を出す。
 * 絞り込み（スポーツ・経過時間の下限）は API の `sport` / `min_elapsed_hours` に渡し、経過時間の数え方と並び
 * （送った時刻の古い順）は契約の正本どおり API が済ませているので、返った順と値のまま出す。
 * 経過が 48 時間以上の行は、予約の画面の「開始まで 24 時間以内」と同じやり方（行の薄い赤と札）で目立たせ、
 * 一覧の上にその件数を出す（今出している一覧の中で数える）。
 */
export function PendingVideoReviewList({
  api,
}: {
  api: Pick<HeadquartersApi, "listPendingVideoReviews">;
}) {
  const [filter, setFilter] = useState<Filter>(NO_FILTER);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchList = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listPendingVideoReviews({
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.minElapsedHours === null ? {} : { min_elapsed_hours: next.minElapsedHours }),
      });
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
    void fetchList(NO_FILTER).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchList]);

  async function changeFilter(next: Filter) {
    setFilter(next);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchList(next);
    if (request === latestRequest.current) setState(loaded);
  }

  const filtered = filter.sport !== "" || filter.minElapsedHours !== null;
  const overdueCount = state.kind === "loaded" ? state.list.videos.filter(isOverdue).length : 0;

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒が送った動画のうち、コーチがまだ添削を返していないものを、送った時刻の古い順（経過の長い順）に見られます。
      </p>
      <div className="flex flex-wrap items-end gap-4">
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded border px-2 py-1"
            value={filter.sport}
            onChange={(event) =>
              void changeFilter({ ...filter, sport: event.target.value as Sport | "" })
            }
          >
            <option value="">すべて</option>
            {SPORTS.map((sport) => (
              <option key={sport} value={sport}>
                {SPORT_LABELS[sport]}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          送ってからの経過
          <select
            className="mt-1 block rounded border px-2 py-1"
            value={filter.minElapsedHours === null ? "" : String(filter.minElapsedHours)}
            onChange={(event) =>
              void changeFilter({
                ...filter,
                minElapsedHours: event.target.value === "" ? null : Number(event.target.value),
              })
            }
          >
            <option value="">すべて</option>
            {MIN_ELAPSED_HOURS_OPTIONS.map((hours) => (
              <option key={hours} value={hours}>
                {hours} 時間以上
              </option>
            ))}
          </select>
        </label>
      </div>
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <p className="text-sm text-gray-500">
          {formatDateTime(state.list.as_of)} 時点の経過時間です（1 時間に満たない端数は切り捨て）。
        </p>
      )}
      {overdueCount > 0 && (
        <p className="rounded border border-red-300 bg-red-50 p-2 text-sm text-red-700">
          {OVERDUE_ELAPSED_HOURS} 時間以上たったものが {overdueCount} 件あります。
        </p>
      )}
      {state.kind === "loaded" && state.list.videos.length === 0 && (
        <p className="text-sm">
          {filtered ? "条件に合う返却待ちの動画添削はありません" : "返却を待っている動画添削はありません"}
        </p>
      )}
      {state.kind === "loaded" && state.list.videos.length > 0 && (
        <table aria-label="返却待ちの動画添削の一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">生徒</th>
              <th className="py-1">担当のコーチ</th>
              <th className="py-1">スポーツ</th>
              <th className="py-1">送った日時</th>
              <th className="py-1">経過</th>
            </tr>
          </thead>
          <tbody>
            {state.list.videos.map((video) => {
              const overdue = isOverdue(video);
              return (
                <tr key={video.video_id} className={`border-t ${overdue ? "bg-red-50" : ""}`}>
                  <td className="py-1">{video.student_display_name ?? "（名前を表示できません）"}</td>
                  <td className="py-1">{video.coach_display_name ?? "（名前を表示できません）"}</td>
                  <td className="py-1">{SPORT_LABELS[video.sport]}</td>
                  <td className="py-1">{formatDateTime(video.submitted_at)}</td>
                  <td className="py-1">
                    {video.elapsed_hours} 時間
                    {overdue && (
                      <span className="ml-2 rounded bg-red-600 px-2 text-xs text-white">
                        {OVERDUE_ELAPSED_HOURS} 時間超
                      </span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}
