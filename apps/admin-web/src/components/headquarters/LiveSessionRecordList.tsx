"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { Result } from "@/api/call";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime, formatDay } from "@/components/labels";

import { type Period, periodError, recentPeriod } from "./bookingPeriod";

type Filter = Period & { coachId: string };

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["LiveLessonSessionRecordList"] }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";

/** 終了の記録がない（通話中か、終了し忘れ）セッションの「終了」の欄。 */
const NO_END_RECORD = "終了の記録なし";

/**
 * サーバーの 422 は ErrorCode の汎用の文言しか出せないので、契約の 422 の条件（期間とコーチの指定）を添えて直す場所を伝える。
 * 送る前の確かめ（periodError）を通っているので、ふつうはここに来ない。
 */
const VALIDATION_HINT = "期間は 31 日までで、終了日は開始日と同じか後の日付にしてください。コーチは一覧から選び直してください。";

function failureMessage(response: Extract<Result<unknown>, { ok: false }>): string {
  return response.code === "validation_failed" ? `${response.message}${VALIDATION_HINT}` : response.message;
}

/** 「終了」の欄: 終了の日時と長さ（分）。終了の記録がなければ「終了の記録なし」。 */
function endLabel(session: Schemas["LiveLessonSessionRecord"]): string {
  if (session.ended_at === null) return NO_END_RECORD;
  const ended = formatDateTime(session.ended_at);
  return session.duration_minutes === null ? ended : `${ended}（${session.duration_minutes} 分）`;
}

/**
 * 本部の「ライブレッスンの記録」。開始した日付（日本時間）の期間と任意のコーチで、通話セッションの記録
 * （listLiveLessonSessions）を読み、コーチ・生徒・開始・終了を出す。既定の期間は今日までの 7 日でコーチはすべて。
 * 対象・並べ方は契約の正本どおり API が済ませているので、返った順（開始の新しい順）のまま出し、使った期間は応答の値を出す。
 * コーチの選択肢はコーチの一覧（listCoaches）の返った順。照会だけで、セッションを終わらせる操作はない。
 */
export function LiveSessionRecordList({
  api,
}: {
  api: Pick<HeadquartersApi, "listLiveLessonSessions" | "listCoaches">;
}) {
  const [draft, setDraft] = useState<Filter>(() => ({ ...recentPeriod(Date.now()), coachId: "" }));
  const [draftError, setDraftError] = useState<string | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  // 条件を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);
  const initialFilter = useRef(draft);

  const fetchSessions = useCallback(
    async ({ from, to, coachId }: Filter): Promise<State> => {
      const response = await api.listLiveLessonSessions({
        from,
        to,
        ...(coachId === "" ? {} : { coach_id: coachId }),
      });
      return response.ok
        ? { kind: "loaded", list: response.data }
        : { kind: "failed", message: failureMessage(response) };
    },
    [api],
  );

  // 開いたときの読み込み（既定の期間・すべてのコーチ）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchSessions(initialFilter.current).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchSessions]);

  // コーチの選択肢。読めなくても記録は期間だけで引けるようにし、コーチの絞り込みだけを使えなくする。
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
    const error = periodError(draft);
    setDraftError(error);
    if (error !== null) return;
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchSessions(draft);
    if (request === latestRequest.current) setState(result);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        通話を開始した日付（日本時間）とコーチで、ライブレッスンがいつ始まりいつ終わったかを開始の新しい順に見られます。期間は 31
        日まで選べます。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submit(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded border px-2 py-1"
            value={draft.from}
            onChange={(event) => setDraft({ ...draft, from: event.target.value })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded border px-2 py-1"
            value={draft.to}
            onChange={(event) => setDraft({ ...draft, to: event.target.value })}
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
            {coaches.kind === "loaded" &&
              coaches.coaches.map((coach) => (
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
      {state.kind === "loaded" && <LoadedSessions list={state.list} />}
    </div>
  );
}

function LoadedSessions({ list }: { list: Schemas["LiveLessonSessionRecordList"] }) {
  return (
    <div className="space-y-4">
      <p className="text-sm">
        {formatDay(list.from)} 〜 {formatDay(list.to)}・{list.sessions.length} 件
      </p>
      {list.sessions.length === 0 && <p className="text-sm">この条件の通話の記録はありません</p>}
      {list.sessions.length > 0 && (
        <table aria-label="ライブレッスンの記録の一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">コーチ</th>
              <th className="py-1">生徒</th>
              <th className="py-1">開始</th>
              <th className="py-1">終了</th>
            </tr>
          </thead>
          <tbody>
            {list.sessions.map((session) => (
              <tr key={session.session_id} className="border-t">
                <td className="py-1">{session.coach_display_name ?? NO_NAME}</td>
                <td className="py-1">{session.student_display_name ?? NO_NAME}</td>
                <td className="py-1">{formatDateTime(session.started_at)}</td>
                <td className={`py-1 ${session.ended_at === null ? "text-gray-500" : ""}`}>
                  {endLabel(session)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
