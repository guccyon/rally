"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime, formatElapsed } from "@/components/labels";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["OverdueLiveLessonSessionList"] }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

/** 絞り込みの条件。`hours` は経過時間の下限（時間）、`coachId` はコーチの ID（空ならすべてのコーチ）。 */
type Filter = { hours: number; coachId: string };

/** 契約の `older_than_hours` の既定（省いたときの 3 時間）。 */
const DEFAULT_HOURS = 3;

/** 経過時間の下限の選択肢（時間）。どれも契約の範囲（1〜168）の中なので 422 にならない。 */
const HOURS_OPTIONS = [1, 3, 6, 12, 24, 72, 168];

function hoursLabel(hours: number): string {
  return hours >= 24 && hours % 24 === 0 ? `${hours / 24} 日（${hours} 時間）以上` : `${hours} 時間以上`;
}

const NO_NAME = "（名前を表示できません）";

/** URL のコーチの ID がコーチの一覧にない（消えたコーチ・打ち間違い）ときの選択肢の名前。 */
const UNKNOWN_COACH = "（一覧にないコーチ）";

/**
 * サーバーの 422 は ErrorCode の汎用の文言しか出せないので、契約の 422 の条件を添えて直す場所を伝える。
 * 経過時間は選択肢が契約の範囲の中だけなので、ふつうは URL のコーチの ID が UUID でないときに来る。
 */
const VALIDATION_HINT = "コーチは一覧から選び直してください。";

function failureMessage(response: Extract<Result<unknown>, { ok: false }>): string {
  return response.code === "validation_failed" ? `${response.message}${VALIDATION_HINT}` : response.message;
}

/**
 * 本部の「終わっていない通話」。始まったまま終了の記録がない通話セッションの一覧（listOverdueLiveLessonSessions）を、
 * 開いたときと経過時間の下限・コーチを変えるたびに読み、コーチ・生徒・開始の日時・開始からの経過を出す。対象・並べ方は契約の
 * 正本どおり API が済ませているので、返った順（開始の古い順）のまま出し、使った下限は応答の値を添える。
 * コーチの選択肢はコーチの一覧（listCoaches）の返った順。`initialCoachId`（URL のクエリ）のコーチで絞って開き、
 * コーチを変えたら `onCoachChange` で知らせる（URL に残すのは呼ぶ側）。照会だけで、セッションを終わらせる操作はない。
 */
export function OverdueLiveSessionList({
  api,
  initialCoachId = "",
  onCoachChange,
}: {
  api: Pick<HeadquartersApi, "listOverdueLiveLessonSessions" | "listCoaches">;
  initialCoachId?: string;
  onCoachChange?: (coachId: string) => void;
}) {
  const [filter, setFilter] = useState<Filter>(() => ({ hours: DEFAULT_HOURS, coachId: initialCoachId }));
  const [state, setState] = useState<State>({ kind: "loading" });
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  // 条件を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchSessions = useCallback(
    async ({ hours, coachId }: Filter): Promise<State> => {
      const response = await api.listOverdueLiveLessonSessions({
        older_than_hours: hours,
        ...(coachId === "" ? {} : { coach_id: coachId }),
      });
      return response.ok
        ? { kind: "loaded", list: response.data }
        : { kind: "failed", message: failureMessage(response) };
    },
    [api],
  );

  // 開いたときの読み込み（既定の下限と、渡されたコーチ）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchSessions({ hours: DEFAULT_HOURS, coachId: initialCoachId }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchSessions, initialCoachId]);

  // コーチの選択肢。読めなくても通話は経過時間だけで引けるようにし、コーチの絞り込みだけを使えなくする。
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
    if (next.coachId !== filter.coachId) onCoachChange?.(next.coachId);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchSessions(next);
    if (request === latestRequest.current) setState(loaded);
  }

  const unknownCoach =
    filter.coachId !== "" &&
    coaches.kind === "loaded" &&
    !coaches.coaches.some((coach) => coach.coach_id === filter.coachId);

  return (
    <div className="space-y-4">
      <p className="text-sm">
        始まったまま終了の記録がない通話を、開始の古い順に見られます。終了し忘れや通話の切断で残ったものを見つけるための一覧です。コーチを選ぶと、そのコーチの通話だけに絞ります。
      </p>
      <div className="flex flex-wrap items-end gap-4">
        <label className="block">
          開始からの経過
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.hours}
            onChange={(event) => void changeFilter({ ...filter, hours: Number(event.target.value) })}
          >
            {HOURS_OPTIONS.map((value) => (
              <option key={value} value={value}>
                {hoursLabel(value)}
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
            {unknownCoach && <option value={filter.coachId}>{UNKNOWN_COACH}</option>}
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
          開始から {state.list.older_than_hours} 時間以上たって、まだ終わっていない通話です。経過は読み込んだ時点のものです。
        </p>
      )}
      {state.kind === "loaded" && state.list.sessions.length === 0 && (
        <p className="text-sm">
          {filter.coachId === "" ? "終わっていない通話はありません" : "このコーチの終わっていない通話はありません"}
        </p>
      )}
      {state.kind === "loaded" && state.list.sessions.length > 0 && (
        <table aria-label="終わっていない通話の一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">コーチ</th>
              <th className="py-1">生徒</th>
              <th className="py-1">開始</th>
              <th className="py-1">開始からの経過</th>
            </tr>
          </thead>
          <tbody>
            {state.list.sessions.map((session) => (
              <tr key={session.session_id} className="border-t">
                <td className="py-1">{session.coach_display_name ?? NO_NAME}</td>
                <td className="py-1">{session.student_display_name ?? NO_NAME}</td>
                <td className="py-1">{formatDateTime(session.started_at)}</td>
                <td className="py-1">{formatElapsed(session.elapsed_sec)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
