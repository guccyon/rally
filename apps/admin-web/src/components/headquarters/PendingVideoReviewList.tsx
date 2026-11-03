"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { Result } from "@/api/call";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { SPORT_LABELS, formatDateTime } from "@/components/labels";

type Sport = Schemas["Sport"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["PendingVideoReviewList"] }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

/**
 * sport・coachId・studentId が空なら、minElapsedHours が null なら、それで絞らない。
 * 生徒は返した動画添削の記録と同じく絞り込みの欄では選ばず、URL のクエリ（生徒の利用の様子の画面からの入口）で受け取る。
 */
export type PendingVideoReviewFilter = {
  sport: Sport | "";
  minElapsedHours: number | null;
  coachId: string;
  studentId: string;
};
type Filter = PendingVideoReviewFilter;

/** スポーツの選択肢（契約の値）。URL のクエリの確かめにも使うので、凍結して公開する。 */
export const SPORTS: readonly Sport[] = Object.freeze(Object.keys(SPORT_LABELS) as Sport[]);

/** 経過時間の下限の選択肢（時間）。URL のクエリの確かめにも使うので、凍結して公開する。 */
export const MIN_ELAPSED_HOURS_OPTIONS = Object.freeze([24, 48, 72, 168] as const);

const NO_FILTER: Filter = { sport: "", minElapsedHours: null, coachId: "", studentId: "" };

/**
 * サーバーの 422 は ErrorCode の汎用の文言しか出せないので、契約の 422 の条件（スポーツ・経過時間・コーチの指定）を添えて直す場所を伝える。
 * 選択肢から選んでいればふつうは来ない（URL のクエリを手で書き換えたときなど）。
 */
const VALIDATION_HINT = "スポーツ・送ってからの経過・コーチは一覧から選び直してください。";
// 生徒で絞っているとき（生徒の ID は URL のクエリから来るので、書き換えられて UUID でないこともある）に添える。
const VALIDATION_HINT_WITH_STUDENT = `${VALIDATION_HINT}生徒は生徒の一覧から選び直してください。`;

function failureMessage(response: Extract<Result<unknown>, { ok: false }>, filter: Filter): string {
  if (response.code !== "validation_failed") return response.message;
  return `${response.message}${filter.studentId === "" ? VALIDATION_HINT : VALIDATION_HINT_WITH_STUDENT}`;
}

/**
 * 0 件のときの文言。コーチだけ・生徒だけで絞っていればそのコーチ・その生徒と言い、ほかの条件もある・コーチと生徒の両方で
 * 絞っていれば「条件に合う」と言う（返した動画添削の記録と同じ）。
 */
function emptyMessageOf({ sport, minElapsedHours, coachId, studentId }: Filter): string {
  if (sport !== "" || minElapsedHours !== null || (coachId !== "" && studentId !== "")) {
    return "条件に合う返却待ちの動画添削はありません";
  }
  if (coachId !== "") return "このコーチの返却待ちの動画添削はありません";
  if (studentId !== "") return "この生徒の返却待ちの動画添削はありません";
  return "返却を待っている動画添削はありません";
}

/** 送ってからの経過がこの時間以上の行を目立たせる（時間。API の `elapsed_hours` と比べる）。 */
export const OVERDUE_ELAPSED_HOURS = 48;

function isOverdue(video: { elapsed_hours: number }): boolean {
  return video.elapsed_hours >= OVERDUE_ELAPSED_HOURS;
}

/**
 * 本部の「返却待ちの動画添削」。コーチがまだ返していない動画添削の一覧（listPendingVideoReviews）を出す。
 * 絞り込み（スポーツ・経過時間の下限・コーチ）は API の `sport` / `min_elapsed_hours` / `coach_id` に渡し、経過時間の数え方と並び
 * （送った時刻の古い順）は契約の正本どおり API が済ませているので、返った順と値のまま出す。
 * 経過が 48 時間以上の行は、予約の画面の「開始まで 24 時間以内」と同じやり方（行の薄い赤と札）で目立たせ、
 * 一覧の上にその件数を出す（今出している一覧の中で数える）。
 * コーチの選択肢はライブレッスンの記録と同じくコーチの一覧（listCoaches）の返った順。
 * 生徒（`studentId`）は API の `student_id` に渡す。欄はなく、スポーツ・経過時間・コーチを変えて読み直しても絞り込みは残る。
 * `initialFilter`（URL のクエリ）の条件で開き、絞り込みを変えたら `onFilterChange` で知らせる（URL に残すのは呼ぶ側）。
 */
export function PendingVideoReviewList({
  api,
  initialFilter = NO_FILTER,
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listPendingVideoReviews" | "listCoaches">;
  initialFilter?: Filter;
  onFilterChange?: (filter: Filter) => void;
}) {
  const [filter, setFilter] = useState<Filter>(initialFilter);
  const [state, setState] = useState<State>({ kind: "loading" });
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);
  const initialFilterRef = useRef(initialFilter);

  const fetchList = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listPendingVideoReviews({
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.minElapsedHours === null ? {} : { min_elapsed_hours: next.minElapsedHours }),
        ...(next.coachId === "" ? {} : { coach_id: next.coachId }),
        ...(next.studentId === "" ? {} : { student_id: next.studentId }),
      });
      return response.ok
        ? { kind: "loaded", list: response.data }
        : { kind: "failed", message: failureMessage(response, next) };
    },
    [api],
  );

  // 開いたときの読み込み（渡された条件。なければ条件なし）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchList(initialFilterRef.current).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchList]);

  // コーチの選択肢。読めなくても一覧はスポーツと経過時間で引けるようにし、コーチの絞り込みだけを使えなくする。
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

  async function changeFilter(next: Filter) {
    setFilter(next);
    onFilterChange?.(next);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchList(next);
    if (request === latestRequest.current) setState(loaded);
  }

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
            className="mt-1 block rounded-md border px-2 py-1"
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
            className="mt-1 block rounded-md border px-2 py-1"
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
        <label className="block">
          コーチ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.coachId}
            disabled={coaches.kind !== "loaded"}
            onChange={(event) => void changeFilter({ ...filter, coachId: event.target.value })}
          >
            <option value="">すべて</option>
            {coaches.kind === "loaded" &&
              coaches.coaches.map((coach) => (
                <option key={coach.coach_id} value={coach.coach_id}>
                  {coach.display_name}
                </option>
              ))}
          </select>
        </label>
      </div>
      {coaches.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `コーチの一覧を読めませんでした。${coaches.message}` }}
        />
      )}
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <p className="text-sm text-text-muted">
          {formatDateTime(state.list.as_of)} 時点の経過時間です（1 時間に満たない端数は切り捨て）。
        </p>
      )}
      {overdueCount > 0 && (
        <p className="rounded-md border border-danger bg-danger-container p-2 text-sm text-danger">
          {OVERDUE_ELAPSED_HOURS} 時間以上たったものが {overdueCount} 件あります。
        </p>
      )}
      {state.kind === "loaded" && state.list.videos.length === 0 && (
        <p className="text-sm">{emptyMessageOf(filter)}</p>
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
                <tr key={video.video_id} className={`border-t ${overdue ? "bg-danger-container" : ""}`}>
                  <td className="py-1">{video.student_display_name ?? "（名前を表示できません）"}</td>
                  <td className="py-1">{video.coach_display_name ?? "（名前を表示できません）"}</td>
                  <td className="py-1">{SPORT_LABELS[video.sport]}</td>
                  <td className="py-1">{formatDateTime(video.submitted_at)}</td>
                  <td className="py-1">
                    {video.elapsed_hours} 時間
                    {overdue && (
                      <span className="ml-2 rounded-md bg-danger px-2 text-xs text-on-primary">
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
