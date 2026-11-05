"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { BOOKING_CANCELLER_LABELS, SPORT_LABELS, formatDateTime, formatDay } from "@/components/labels";

import { BookingLessonMenuFilter } from "./BookingLessonMenuFilter";
import { MAX_PERIOD_DAYS, type Period, periodError } from "./bookingPeriod";
import { SPORTS } from "./PendingVideoReviewList";

type Sport = Schemas["Sport"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["CancelledBookingList"]; coachId: string; lessonMenuId: string; sport: Sport | "" }
  | { kind: "failed"; message: string };

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

type LessonMenusState =
  | { kind: "loading" }
  | { kind: "loaded"; lessonMenus: Schemas["CoachLessonMenu"][] }
  | { kind: "failed"; message: string };

/**
 * 期間が空なら API の既定（今日までの 7 日）に任せる。coachId が空ならすべてのコーチ、lessonMenuId が空ならすべてのレッスンメニュー、
 * sport が空ならすべての競技。
 */
export type CancelledBookingFilter = { period: Period | null; coachId: string; lessonMenuId: string; sport: Sport | "" };
type Filter = CancelledBookingFilter;

const NO_NAME = "（名前を表示できません）";

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間・コーチ・レッスンメニュー・スポーツの指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。コーチ・レッスンメニュー・スポーツは一覧から選び直してください。`;

/** レッスンメニューの選択肢の文言。コーチが違えば同じ名前のメニューがあるので、引けるときはコーチの名前を添える。 */
export function lessonMenuOptionLabel(menu: Schemas["CoachLessonMenu"]): string {
  return menu.coach_display_name === undefined ? menu.name : `${menu.name}（${menu.coach_display_name}）`;
}

/**
 * 本部の「キャンセルされた予約」。すべてのコーチの枠への予約のうちキャンセルされたものを、キャンセルした日付（日本時間）の
 * 期間と任意のコーチ・レッスンメニュー・スポーツで出す（listCancelledBookings）。開いたときは期間を付けずに読み、API の既定（今日までの 7 日）で実際に
 * 使った期間を入力欄に入れる。並び（キャンセルの新しい順）は契約の正本どおり API が済ませているので、返った順と値のまま出す。
 * コーチの選択肢はコーチの一覧（listCoaches）の返った順（ビデオ通話の記録と同じ）。レッスンメニューの選択肢はレッスンメニューの
 * 一覧（listLessonMenus。絞り込みなしで、公開をやめたメニューも含む）の返った順。レッスンメニューで絞った一覧には、予約の一覧と
 * 同じく上に「どのレッスンメニューで絞っているか」（BookingLessonMenuFilter）を出す。
 * スポーツの選択欄はほかの競技の絞り込みと同じ形（「すべて」と各スポーツ）で、ビデオ通話の記録と同じく期間・コーチと一緒に
 * 「この条件で見る」で API の `sport` に渡す（予約の競技は枠のコーチの提供スポーツで決まる。契約）。
 * `initialCoachId`・`initialLessonMenuId`・`initialSport`（URL のクエリ）で開き、「この条件で見る」で読み直したら `onFilterChange` で知らせる
 * （URL に残すのは呼ぶ側）。
 */
export function CancelledBookingList({
  api,
  initialCoachId = "",
  initialLessonMenuId = "",
  initialSport = "",
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listCancelledBookings" | "listCoaches" | "listLessonMenus">;
  initialCoachId?: string;
  initialLessonMenuId?: string;
  initialSport?: Sport | "";
  onFilterChange?: (filter: Filter) => void;
}) {
  // period が null なら API の既定の期間に任せる（開いたときの 1 回だけ）。
  const [draft, setDraft] = useState<Filter>({
    period: null,
    coachId: initialCoachId,
    lessonMenuId: initialLessonMenuId,
    sport: initialSport,
  });
  const [draftError, setDraftError] = useState<string | null>(null);
  // 最後に読みにいったレッスンメニュー（一覧の上の「絞っています」に出す。選んだだけの draft とは分ける）。
  const [appliedLessonMenuId, setAppliedLessonMenuId] = useState(initialLessonMenuId);
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [lessonMenus, setLessonMenus] = useState<LessonMenusState>({ kind: "loading" });
  const [state, setState] = useState<State>({ kind: "loading" });
  // 条件を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchList = useCallback(
    async (filter: Filter): Promise<State> => {
      const response = await api.listCancelledBookings({
        ...(filter.period ?? {}),
        ...(filter.coachId === "" ? {} : { coach_id: filter.coachId }),
        ...(filter.lessonMenuId === "" ? {} : { lesson_menu_id: filter.lessonMenuId }),
        ...(filter.sport === "" ? {} : { sport: filter.sport }),
      });
      if (response.ok) {
        return {
          kind: "loaded",
          list: response.data,
          coachId: filter.coachId,
          lessonMenuId: filter.lessonMenuId,
          sport: filter.sport,
        };
      }
      return {
        kind: "failed",
        message: response.code === "validation_failed" ? VALIDATION_FAILED : response.message,
      };
    },
    [api],
  );

  // 開いたときの読み込み（API の既定の期間と渡されたコーチ・レッスンメニュー・スポーツ）。読めたら実際に使った期間を入力欄に入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchList({
      period: null,
      coachId: initialCoachId,
      lessonMenuId: initialLessonMenuId,
      sport: initialSport,
    }).then((next) => {
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
  }, [fetchList, initialCoachId, initialLessonMenuId, initialSport]);

  // コーチの選択肢。読めなくても一覧は期間だけで引けるようにし、コーチの絞り込みだけを使えなくする。
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

  // レッスンメニューの選択肢。コーチと同じく、読めなくても一覧は引けるようにし、レッスンメニューの絞り込みだけを使えなくする。
  useEffect(() => {
    let active = true;
    void api.listLessonMenus({}).then((response) => {
      if (!active) return;
      setLessonMenus(
        response.ok
          ? { kind: "loaded", lessonMenus: response.data.lesson_menus }
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
    const applied = { ...draft, period };
    onFilterChange?.(applied);
    setAppliedLessonMenuId(applied.lessonMenuId);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchList(applied);
    if (request === latestRequest.current) setState(result);
  }

  const period = draft.period ?? { from: "", to: "" };

  return (
    <div className="space-y-4">
      <p className="text-sm">
        すべてのコーチの枠への予約のうちキャンセルされたものを、キャンセルした日付（日本時間）・コーチ・レッスンメニュー・スポーツで絞って、キャンセルの新しい順に見られます。期間は{" "}
        {MAX_PERIOD_DAYS} 日まで選べます。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submit(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={period.from}
            onChange={(event) => setDraft({ ...draft, period: { ...period, from: event.target.value } })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={period.to}
            onChange={(event) => setDraft({ ...draft, period: { ...period, to: event.target.value } })}
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
      {appliedLessonMenuId !== "" && (
        <BookingLessonMenuFilter key={appliedLessonMenuId} api={api} lessonMenuId={appliedLessonMenuId} />
      )}
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <LoadedBookings
          list={state.list}
          sport={state.sport}
          emptyMessage={emptyMessageOf(state.coachId, state.lessonMenuId, state.sport)}
        />
      )}
    </div>
  );
}

/** 0 件のときの文言。コーチ・レッスンメニュー・スポーツで絞っていれば、そのコーチの枠・そのメニュー・その競技のことだと言う。 */
function emptyMessageOf(coachId: string, lessonMenuId: string, sport: Sport | ""): string {
  const parts = [
    ...(sport === "" ? [] : [SPORT_LABELS[sport]]),
    ...(coachId === "" ? [] : ["このコーチの枠"]),
    ...(lessonMenuId === "" ? [] : ["このレッスンメニュー"]),
  ];
  return parts.length === 0
    ? "この期間にキャンセルされた予約はありません"
    : `この期間に${parts.join("の")}でキャンセルされた予約はありません`;
}

function LoadedBookings({
  list,
  sport,
  emptyMessage,
}: {
  list: Schemas["CancelledBookingList"];
  sport: Sport | "";
  emptyMessage: string;
}) {
  return (
    <div className="space-y-2">
      <p className="text-sm">
        {formatDay(list.from)} 〜 {formatDay(list.to)}・{sport === "" ? "" : `${SPORT_LABELS[sport]} `}
        {list.bookings.length} 件
      </p>
      {list.bookings.length === 0 && <p className="text-sm">{emptyMessage}</p>}
      {list.bookings.length > 0 && (
        <table aria-label="キャンセルされた予約の一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">キャンセルした日時</th>
              <th className="py-1">枠の開始</th>
              <th className="py-1">コーチ</th>
              <th className="py-1">生徒</th>
              <th className="py-1">キャンセルした人</th>
            </tr>
          </thead>
          <tbody>
            {list.bookings.map((booking) => (
              <tr key={booking.booking_id} className="border-t">
                <td className="py-1">{formatDateTime(booking.cancelled_at)}</td>
                <td className="py-1">{formatDateTime(booking.starts_at)}</td>
                <td className="py-1">{booking.coach_display_name ?? NO_NAME}</td>
                <td className="py-1">{booking.student_display_name ?? NO_NAME}</td>
                <td className="py-1">
                  {booking.cancelled_by === null ? "記録なし" : BOOKING_CANCELLER_LABELS[booking.cancelled_by]}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
