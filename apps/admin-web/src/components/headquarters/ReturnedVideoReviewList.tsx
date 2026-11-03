"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { SPORT_LABELS, formatDateTime, formatDay } from "@/components/labels";

import { BookingLessonMenuFilter } from "./BookingLessonMenuFilter";
import { MAX_PERIOD_DAYS, type Period, periodError } from "./bookingPeriod";
import { lessonMenuOptionLabel } from "./CancelledBookingList";

type Sport = Schemas["Sport"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["ReturnedVideoReviewList"]; filter: Filter }
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
 * 期間が空なら API の既定（今日までの 7 日）に任せる。coachId が空ならすべてのコーチ、sport が空ならすべてのスポーツ、
 * studentId が空ならすべての生徒、lessonMenuId が空ならすべてのレッスンメニュー。生徒は絞り込みの欄では選ばず、URL のクエリ（生徒の利用の様子の画面からの入口）で受け取る。
 */
export type ReturnedVideoReviewFilter = {
  period: Period | null;
  coachId: string;
  sport: Sport | "";
  studentId: string;
  lessonMenuId: string;
};
type Filter = ReturnedVideoReviewFilter;

const NO_FILTER: Filter = { period: null, coachId: "", sport: "", studentId: "", lessonMenuId: "" };

/** スポーツの選択肢（契約の値）。URL のクエリの確かめにも使うので、凍結して公開する。 */
export const SPORTS: readonly Sport[] = Object.freeze(Object.keys(SPORT_LABELS) as Sport[]);

const NO_NAME = "（名前を表示できません）";

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
// レッスンメニューは選択肢から選んでいればふつうは来ない（URL のクエリを手で書き換えたときなど）。
const VALIDATION_FAILED = `期間・コーチ・レッスンメニューのどれかの指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。コーチとレッスンメニューは一覧から選び直してください。`;
// 生徒で絞っているとき（生徒の ID は URL のクエリから来るので、書き換えられて UUID でないこともある）の 422 の理由。
const VALIDATION_FAILED_WITH_STUDENT = `期間・コーチ・レッスンメニュー・生徒のどれかの指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。コーチとレッスンメニューは一覧から選び直してください。生徒は生徒の一覧から選び直してください。`;

/**
 * 本部の「返した動画添削」。コーチが返した動画添削を、返却の日付（日本時間）の期間・コーチ・スポーツ・レッスンメニューで絞って出す
 * （listReturnedVideoReviews）。開いたときは期間を付けずに読み、API の既定（今日までの 7 日）で実際に使った期間を入力欄に入れる。
 * かかった時間の数え方と並び（返却の新しい順）は契約の正本どおり API が済ませているので、返った順と値のまま出す。
 * スポーツは API の `sport` に渡す（期間・コーチと同じく「この条件で見る」で読み直す）。
 * レッスンメニューは API の `lesson_menu_id` に渡し、動画を送った時点のメニューで絞る（契約）。選択肢と文言は返却待ちの動画添削に
 * 揃える（レッスンメニューの一覧 listLessonMenus を絞り込みなしで読み、非同期のメニューを返った順に出す）。「この条件で見る」で読んだ
 * 条件がメニューで絞っていれば、一覧の上に「どのレッスンメニューで絞っているか」（BookingLessonMenuFilter）を出す（0 件のときも出す）。
 * 生徒（`studentId`）は API の `student_id` に渡す。欄はなく、期間・コーチ・スポーツ・レッスンメニューを変えて読み直しても絞り込みは残る。
 * `initialFilter`（URL のクエリ）の条件で開き、「この条件で見る」で読み直したら `onFilterChange` で知らせる（URL に残すのは呼ぶ側）。
 */
export function ReturnedVideoReviewList({
  api,
  initialFilter = NO_FILTER,
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listReturnedVideoReviews" | "listCoaches" | "listLessonMenus">;
  initialFilter?: Filter;
  onFilterChange?: (filter: Filter) => void;
}) {
  const [draft, setDraft] = useState<Filter>(initialFilter);
  const [draftError, setDraftError] = useState<string | null>(null);
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  const [lessonMenus, setLessonMenus] = useState<LessonMenusState>({ kind: "loading" });
  // 一覧の上に名前を出すのは、入力中の欄ではなく最後に読んだ条件のメニュー。
  const [appliedLessonMenuId, setAppliedLessonMenuId] = useState(initialFilter.lessonMenuId);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 条件を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchList = useCallback(
    async (filter: Filter): Promise<State> => {
      const response = await api.listReturnedVideoReviews({
        ...(filter.period ?? {}),
        ...(filter.coachId === "" ? {} : { coach_id: filter.coachId }),
        ...(filter.sport === "" ? {} : { sport: filter.sport }),
        ...(filter.studentId === "" ? {} : { student_id: filter.studentId }),
        ...(filter.lessonMenuId === "" ? {} : { lesson_menu_id: filter.lessonMenuId }),
      });
      if (response.ok) {
        return { kind: "loaded", list: response.data, filter };
      }
      if (response.code === "validation_failed") {
        return {
          kind: "failed",
          message: filter.studentId === "" ? VALIDATION_FAILED : VALIDATION_FAILED_WITH_STUDENT,
        };
      }
      return { kind: "failed", message: response.message };
    },
    [api],
  );

  // 開いたときの読み込み（渡された条件。期間がなければ API の既定）。読めたら実際に使った期間を入力欄に入れる。
  const {
    period: initialPeriod,
    coachId: initialCoachId,
    sport: initialSport,
    studentId: initialStudentId,
    lessonMenuId: initialLessonMenuId,
  } = initialFilter;
  const initialFrom = initialPeriod?.from;
  const initialTo = initialPeriod?.to;
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    const period =
      initialFrom === undefined || initialTo === undefined ? null : { from: initialFrom, to: initialTo };
    void fetchList({
      period,
      coachId: initialCoachId,
      sport: initialSport,
      studentId: initialStudentId,
      lessonMenuId: initialLessonMenuId,
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
  }, [fetchList, initialFrom, initialTo, initialCoachId, initialSport, initialStudentId, initialLessonMenuId]);

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

  // レッスンメニューの選択肢。読めなくても記録の一覧は出し、レッスンメニューの絞り込みだけを使えなくする。
  useEffect(() => {
    let active = true;
    void api.listLessonMenus({}).then((response) => {
      if (!active) return;
      setLessonMenus(
        response.ok
          ? {
              kind: "loaded",
              lessonMenus: response.data.lesson_menus.filter((menu) => menu.lesson_type === "async_lesson"),
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
            {coachOptions.map((coach) => (
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
      {appliedLessonMenuId !== "" && (
        <BookingLessonMenuFilter key={appliedLessonMenuId} api={api} lessonMenuId={appliedLessonMenuId} />
      )}
      {draftError !== null && <ResultMessage result={{ ok: false, message: draftError }} />}
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <LoadedReviews list={state.list} emptyMessage={emptyMessageOf(state.filter)} />
      )}
    </div>
  );
}

/**
 * 0 件のときの文言。スポーツで絞っている・コーチと生徒とレッスンメニューのうち 2 つ以上で絞っていれば、返却待ちの画面と同じく
 * 「条件に合う」と言う（どのメニューかは一覧の上の BookingLessonMenuFilter が名前で出す）。
 */
function emptyMessageOf({ coachId, sport, studentId, lessonMenuId }: Filter): string {
  const narrowed = [coachId !== "", studentId !== "", lessonMenuId !== ""].filter(Boolean).length;
  if (sport !== "" || narrowed >= 2) return "この期間に条件に合う返した動画添削はありません";
  if (lessonMenuId !== "") return "この期間にこのレッスンメニューの返した動画添削はありません";
  if (coachId !== "") return "この期間にこのコーチが返した動画添削はありません";
  if (studentId !== "") return "この期間にこの生徒に返した動画添削はありません";
  return "この期間に返した動画添削はありません";
}

function LoadedReviews({
  list,
  emptyMessage,
}: {
  list: Schemas["ReturnedVideoReviewList"];
  emptyMessage: string;
}) {
  return (
    <div className="space-y-2">
      <p className="text-sm">
        {formatDay(list.from)} 〜 {formatDay(list.to)}・{list.reviews.length} 件
      </p>
      <p className="text-sm text-text-muted">
        かかった時間は送った時刻から返した時刻までです（1 時間に満たない端数は切り捨て）。
      </p>
      {list.reviews.length === 0 && (
        <p className="text-sm">{emptyMessage}</p>
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
