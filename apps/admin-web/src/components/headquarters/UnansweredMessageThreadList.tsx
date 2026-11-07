"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { Result } from "@/api/call";
import type { HeadquartersApi } from "@/api/headquarters";
import { DataTable, type DataTableColumn } from "@/components/ui/DataTable";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime } from "@/components/labels";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["UnansweredMessageThreadList"] }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

/**
 * olderThanHours が null なら API に渡さない（契約の既定の 24 時間）。coachId・studentId が空なら、それで絞らない。
 * 生徒は返却待ちの動画添削と同じく絞り込みの欄では選ばず、URL のクエリ（生徒の利用の様子の画面からの入口）で受け取る。
 */
export type UnansweredMessageThreadFilter = {
  olderThanHours: number | null;
  coachId: string;
  studentId: string;
};
type Filter = UnansweredMessageThreadFilter;

/** 契約の `older_than_hours` の既定（省いたときに API が使う時間）。 */
export const DEFAULT_OLDER_THAN_HOURS = 24;

/**
 * 返事を待っている時間の選択肢（時間）。返却待ちの動画添削の経過の選択肢と同じ値で、先頭が契約の既定。
 * URL のクエリの確かめにも使うので、凍結して公開する。
 */
export const OLDER_THAN_HOURS_OPTIONS = Object.freeze([DEFAULT_OLDER_THAN_HOURS, 48, 72, 168] as const);

const NO_FILTER: Filter = { olderThanHours: null, coachId: "", studentId: "" };

/**
 * サーバーの 422 は ErrorCode の汎用の文言しか出せないので、契約の 422 の条件（時間・コーチ・生徒の指定）を添えて直す場所を伝える。
 * 選択肢から選んでいればふつうは来ない（URL のクエリを手で書き換えたときなど）。
 */
const VALIDATION_HINT = "返事を待っている時間・コーチは一覧から選び直してください。";
// 生徒で絞っているとき（生徒の ID は URL のクエリから来るので、書き換えられて UUID でないこともある）に添える。
const VALIDATION_HINT_WITH_STUDENT = `${VALIDATION_HINT}生徒は生徒の一覧から選び直してください。`;

function failureMessage(response: Extract<Result<unknown>, { ok: false }>, filter: Filter): string {
  if (response.code !== "validation_failed") return response.message;
  return `${response.message}${filter.studentId === "" ? VALIDATION_HINT : VALIDATION_HINT_WITH_STUDENT}`;
}

/**
 * 0 件のときの文言。絞り込みの条件が分かるように、API が実際に使った時間（応答の `older_than_hours`）と、
 * コーチ・生徒で絞っていればそのコーチ・その生徒であることを言う。
 */
function emptyMessageOf({ coachId, studentId }: Filter, olderThanHours: number): string {
  const condition = `返事を待って ${olderThanHours} 時間以上たったメッセージはありません`;
  if (coachId !== "" && studentId !== "") return `このコーチとこの生徒のやり取りに、${condition}`;
  if (coachId !== "") return `このコーチのやり取りに、${condition}`;
  if (studentId !== "") return `この生徒のやり取りに、${condition}`;
  return condition;
}

/** 経過の秒（API の `elapsed_sec`）を、返却待ちの動画添削と同じく時間に切り捨てて出す。 */
function elapsedHoursOf(thread: { elapsed_sec: number }): number {
  return Math.floor(thread.elapsed_sec / 3600);
}

/** 返事のないメッセージの表の列。 */
const THREAD_COLUMNS: DataTableColumn<Schemas["UnansweredMessageThread"]>[] = [
  { header: "生徒", cell: (thread) => thread.student_display_name ?? "（名前を表示できません）" },
  { header: "担当のコーチ", cell: (thread) => thread.coach_display_name ?? "（名前を表示できません）" },
  { header: "待ち始めた日時", cell: (thread) => formatDateTime(thread.waiting_since) },
  { header: "経過", variant: "numeric", cell: (thread) => `${elapsedHoursOf(thread)} 時間` },
];

/**
 * 本部の「返事のないメッセージ」。生徒からのメッセージにコーチが返事をしていないスレッドの一覧（listUnansweredMessageThreads）を出す。
 * 絞り込み（返事を待っている時間・コーチ）は API の `older_than_hours` / `coach_id` に渡し、何を発言と数えるか・経過の数え方・並び
 * （待ち始めた時刻の古い順）は契約の正本どおり API が済ませているので、返った順と値のまま出す。メッセージの本文は契約が返さないので出さない。
 * 部品と振る舞いは返却待ちの動画添削の一覧（PendingVideoReviewList）に揃える。コーチの選択肢はコーチの一覧（listCoaches）の返った順。
 * 生徒（`studentId`）は API の `student_id` に渡す。欄はなく、時間・コーチを変えて読み直しても絞り込みは残る。
 * `initialFilter`（URL のクエリ）の条件で開き、絞り込みを変えたら `onFilterChange` で知らせる（URL に残すのは呼ぶ側）。
 */
export function UnansweredMessageThreadList({
  api,
  initialFilter = NO_FILTER,
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listUnansweredMessageThreads" | "listCoaches">;
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
      const response = await api.listUnansweredMessageThreads({
        ...(next.olderThanHours === null ? {} : { older_than_hours: next.olderThanHours }),
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

  // コーチの選択肢。読めなくても一覧は時間で引けるようにし、コーチの絞り込みだけを使えなくする。
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

  return (
    <div className="space-y-4">
      <p className="text-sm">
        生徒からのメッセージに担当のコーチが返事をしていないやり取りを、待ち始めた時刻の古い順（経過の長い順）に見られます。メッセージの本文は出しません。
      </p>
      <div className="flex flex-wrap items-end gap-4">
        <label className="block">
          返事を待っている時間
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={String(filter.olderThanHours ?? DEFAULT_OLDER_THAN_HOURS)}
            onChange={(event) => {
              const hours = Number(event.target.value);
              void changeFilter({ ...filter, olderThanHours: hours === DEFAULT_OLDER_THAN_HOURS ? null : hours });
            }}
          >
            {OLDER_THAN_HOURS_OPTIONS.map((hours) => (
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
        <p className="text-sm text-text-muted">読み込んだ時点の経過時間です（1 時間に満たない端数は切り捨て）。</p>
      )}
      {state.kind === "loaded" && state.list.threads.length === 0 && (
        <p className="text-sm">{emptyMessageOf(filter, state.list.older_than_hours)}</p>
      )}
      {state.kind === "loaded" && state.list.threads.length > 0 && (
        <DataTable
          label="返事のないメッセージの一覧"
          columns={THREAD_COLUMNS}
          rows={state.list.threads}
          rowKey={(thread) => thread.thread_id}
        />
      )}
    </div>
  );
}
