"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { Result } from "@/api/call";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { SPORT_LABELS, formatDateTime, formatDay } from "@/components/labels";

import { BookingLessonMenuFilter } from "./BookingLessonMenuFilter";
import { type Period, periodError, recentPeriod } from "./bookingPeriod";
import { lessonMenuOptionLabel } from "./CancelledBookingList";
import { SPORTS } from "./PendingVideoReviewList";

type Sport = Schemas["Sport"];

/** coachId が空ならすべてのコーチ、sport が空ならすべての競技、lessonMenuId が空ならすべてのレッスンメニュー。 */
type Filter = Period & { coachId: string; sport: Sport | ""; lessonMenuId: string };

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["LiveLessonSessionRecordList"] }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

type LessonMenusState =
  | { kind: "loading" }
  | { kind: "loaded"; lessonMenus: Schemas["CoachLessonMenu"][] }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";

/** 終了の記録がない（通話中か、終了し忘れ）セッションの「終了」の欄。 */
const NO_END_RECORD = "終了の記録なし";

/**
 * サーバーの 422 は ErrorCode の汎用の文言しか出せないので、契約の 422 の条件（期間・コーチ・スポーツ・レッスンメニューの指定）を添えて直す場所を伝える。
 * 送る前の確かめ（periodError）を通っているので、ふつうはここに来ない。
 */
const VALIDATION_HINT =
  "期間は 31 日までで、終了日は開始日と同じか後の日付にしてください。スポーツ・コーチ・レッスンメニューは一覧から選び直してください。";

/** 生徒で絞っているときに 422 に添える、生徒の指定（URL の `student_id`。UUID でなければ 422）の直し方。 */
const STUDENT_VALIDATION_HINT = "生徒は生徒の一覧から選び直してください。";

function failureMessage(response: Extract<Result<unknown>, { ok: false }>, studentId: string | undefined): string {
  if (response.code !== "validation_failed") return response.message;
  return `${response.message}${VALIDATION_HINT}${studentId === undefined ? "" : STUDENT_VALIDATION_HINT}`;
}

/**
 * 0 件のときの文言。期間のほかにレッスンメニューだけで絞っていればそのメニューと言い（返却待ちの動画添削と同じ。どのメニューかは
 * 一覧の上の BookingLessonMenuFilter が名前で出す）、それ以外は今までどおり「この条件の」と言う。
 */
function emptyMessageOf({ coachId, sport, lessonMenuId }: Filter, studentId: string | undefined): string {
  if (lessonMenuId !== "" && coachId === "" && sport === "" && studentId === undefined) {
    return "このレッスンメニューのビデオ通話の記録はありません";
  }
  return "この条件のビデオ通話の記録はありません";
}

/** 「終了」の欄: 終了の日時と長さ（分）。終了の記録がなければ「終了の記録なし」。 */
function endLabel(session: Schemas["LiveLessonSessionRecord"]): string {
  if (session.ended_at === null) return NO_END_RECORD;
  const ended = formatDateTime(session.ended_at);
  return session.duration_minutes === null ? ended : `${ended}（${session.duration_minutes} 分）`;
}

/**
 * 本部の「ビデオ通話の記録」。開始した日付（日本時間）の期間と任意のコーチ・スポーツ・レッスンメニューで、通話セッションの記録
 * （listLiveLessonSessions）を読み、コーチ・生徒・開始・終了を出す。既定の期間は今日までの 7 日でコーチ・スポーツ・レッスンメニューはすべて。
 * 対象・並べ方は契約の正本どおり API が済ませているので、返った順（開始の新しい順）のまま出し、使った期間は応答の値を出す。
 * コーチの選択肢はコーチの一覧（listCoaches）の返った順。
 * スポーツの選択欄はスポーツごとの予約枠の需給（SlotSupplyReport）と同じく「この条件で見る」で API の `sport` に渡す。照会だけで、セッションを終わらせる操作はない。
 * レッスンメニューはほかの欄と同じく「この条件で見る」で API の `lesson_menu_id` に渡し、予約した時点のメニューで絞る（契約）。欄の部品は
 * 返却待ちの動画添削と同じで、選択肢はレッスンメニューの一覧（listLessonMenus。絞り込みなしで、公開をやめたメニューも含む）のうち
 * 同期のメニュー（予約するときに選べるもの）を返った順に出す。メニューで絞って読んだあいだは一覧の上に「どのレッスンメニューで
 * 絞っているか」（BookingLessonMenuFilter）を出す（0 件のときも出すので、どのメニューで空になったかが分かる）。
 * `studentId` があれば、その生徒の予約が結び付いたセッションだけに絞る（`student_id`）。期間・コーチ・スポーツを変えても絞り込みは残る。
 */
export function LiveSessionRecordList({
  api,
  studentId,
}: {
  api: Pick<HeadquartersApi, "listLiveLessonSessions" | "listCoaches" | "listLessonMenus">;
  studentId?: string;
}) {
  const [draft, setDraft] = useState<Filter>(() => ({
    ...recentPeriod(Date.now()),
    coachId: "",
    sport: "",
    lessonMenuId: "",
  }));
  const [draftError, setDraftError] = useState<string | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [lessonMenus, setLessonMenus] = useState<LessonMenusState>({ kind: "loading" });
  // 最後に読んだ条件（選んだだけで読み直していない欄の値と分けて、絞っている表示と 0 件の文言に使う）。
  const [applied, setApplied] = useState<Filter>(draft);
  // 条件を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);
  const initialFilter = useRef(draft);

  const fetchSessions = useCallback(
    async ({ from, to, coachId, sport, lessonMenuId }: Filter): Promise<State> => {
      const response = await api.listLiveLessonSessions({
        from,
        to,
        ...(coachId === "" ? {} : { coach_id: coachId }),
        ...(sport === "" ? {} : { sport }),
        ...(studentId === undefined ? {} : { student_id: studentId }),
        ...(lessonMenuId === "" ? {} : { lesson_menu_id: lessonMenuId }),
      });
      return response.ok
        ? { kind: "loaded", list: response.data }
        : { kind: "failed", message: failureMessage(response, studentId) };
    },
    [api, studentId],
  );

  // 開いたときの読み込み（既定の期間・すべてのコーチ・すべてのスポーツ・すべてのレッスンメニュー）。初期の状態が loading なので、ここでは結果だけを入れる。
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

  // レッスンメニューの選択肢。読めなくても記録はほかの条件で引けるようにし、レッスンメニューの絞り込みだけを使えなくする。
  useEffect(() => {
    let active = true;
    void api.listLessonMenus({}).then((response) => {
      if (!active) return;
      setLessonMenus(
        response.ok
          ? {
              kind: "loaded",
              lessonMenus: response.data.lesson_menus.filter((menu) => menu.lesson_type === "live_lesson"),
            }
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
    setApplied(draft);
    setState({ kind: "loading" });
    const result = await fetchSessions(draft);
    if (request === latestRequest.current) setState(result);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        {`開始した日付（日本時間）とコーチ・スポーツ・レッスンメニューで、${studentId === undefined ? "" : "この生徒の"}ビデオ通話がいつ始まりいつ終わったかを開始の新しい順に見られます。期間は 31 日まで選べます。`}
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submit(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={draft.from}
            onChange={(event) => setDraft({ ...draft, from: event.target.value })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={draft.to}
            onChange={(event) => setDraft({ ...draft, to: event.target.value })}
          />
        </label>
        <label className="block">
          コーチ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
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
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={draft.sport}
            onChange={(event) => setDraft({ ...draft, sport: event.target.value as Sport | "" })}
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
          レッスンメニュー
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={draft.lessonMenuId}
            disabled={lessonMenus.kind !== "loaded"}
            onChange={(event) => setDraft({ ...draft, lessonMenuId: event.target.value })}
          >
            <option value="">すべて</option>
            {lessonMenus.kind === "loaded" &&
              lessonMenus.lessonMenus.map((menu) => (
                <option key={menu.lesson_menu_id} value={menu.lesson_menu_id}>
                  {lessonMenuOptionLabel(menu)}
                </option>
              ))}
          </select>
        </label>
        <button type="submit" className="rounded-pill bg-primary px-4 py-1 text-on-primary">
          この条件で見る
        </button>
      </form>
      {coaches.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `コーチの一覧を読めませんでした。${coaches.message}` }}
        />
      )}
      {lessonMenus.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `レッスンメニューの一覧を読めませんでした。${lessonMenus.message}` }}
        />
      )}
      {draftError !== null && <ResultMessage result={{ ok: false, message: draftError }} />}
      {applied.lessonMenuId !== "" && (
        <BookingLessonMenuFilter key={applied.lessonMenuId} api={api} lessonMenuId={applied.lessonMenuId} />
      )}
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <LoadedSessions list={state.list} emptyMessage={emptyMessageOf(applied, studentId)} />
      )}
    </div>
  );
}

function LoadedSessions({
  list,
  emptyMessage,
}: {
  list: Schemas["LiveLessonSessionRecordList"];
  emptyMessage: string;
}) {
  return (
    <div className="space-y-4">
      <p className="text-sm">
        {formatDay(list.from)} 〜 {formatDay(list.to)}・{list.sessions.length} 件
      </p>
      {list.sessions.length === 0 && <p className="text-sm">{emptyMessage}</p>}
      {list.sessions.length > 0 && (
        <table aria-label="ビデオ通話の記録の一覧" className="w-full text-left text-sm">
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
                <td className={`py-1 ${session.ended_at === null ? "text-text-muted" : ""}`}>
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
