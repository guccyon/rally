"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime, formatElapsed } from "@/components/labels";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["OverdueLiveLessonSessionList"] }
  | { kind: "failed"; message: string };

/** 契約の `older_than_hours` の既定（省いたときの 3 時間）。 */
const DEFAULT_HOURS = 3;

/** 経過時間の下限の選択肢（時間）。どれも契約の範囲（1〜168）の中なので 422 にならない。 */
const HOURS_OPTIONS = [1, 3, 6, 12, 24, 72, 168];

function hoursLabel(hours: number): string {
  return hours >= 24 && hours % 24 === 0 ? `${hours / 24} 日（${hours} 時間）以上` : `${hours} 時間以上`;
}

const NO_NAME = "（名前を表示できません）";

/**
 * 本部の「終わっていない通話」。始まったまま終了の記録がない通話セッションの一覧（listOverdueLiveLessonSessions）を、
 * 開いたときと経過時間の下限を変えるたびに読み、コーチ・生徒・開始の日時・開始からの経過を出す。対象・並べ方は契約の
 * 正本どおり API が済ませているので、返った順（開始の古い順）のまま出し、使った下限は応答の値を添える。照会だけで、
 * セッションを終わらせる操作はない。
 */
export function OverdueLiveSessionList({
  api,
}: {
  api: Pick<HeadquartersApi, "listOverdueLiveLessonSessions">;
}) {
  const [hours, setHours] = useState(DEFAULT_HOURS);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 下限を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchSessions = useCallback(
    async (olderThanHours: number): Promise<State> => {
      const response = await api.listOverdueLiveLessonSessions({ older_than_hours: olderThanHours });
      return response.ok
        ? { kind: "loaded", list: response.data }
        : { kind: "failed", message: response.message };
    },
    [api],
  );

  // 開いたときの読み込み（既定の下限）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchSessions(DEFAULT_HOURS).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchSessions]);

  async function changeHours(next: number) {
    setHours(next);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchSessions(next);
    if (request === latestRequest.current) setState(loaded);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        始まったまま終了の記録がない通話を、コーチを問わず開始の古い順に見られます。終了し忘れや通話の切断で残ったものを見つけるための一覧です。
      </p>
      <label className="block">
        開始からの経過
        <select
          className="mt-1 block rounded border px-2 py-1"
          value={hours}
          onChange={(event) => void changeHours(Number(event.target.value))}
        >
          {HOURS_OPTIONS.map((value) => (
            <option key={value} value={value}>
              {hoursLabel(value)}
            </option>
          ))}
        </select>
      </label>
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <p className="text-sm text-gray-500">
          開始から {state.list.older_than_hours} 時間以上たって、まだ終わっていない通話です。経過は読み込んだ時点のものです。
        </p>
      )}
      {state.kind === "loaded" && state.list.sessions.length === 0 && (
        <p className="text-sm">終わっていない通話はありません</p>
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
