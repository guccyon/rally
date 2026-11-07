"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { Button } from "@/components/ui/Button";
import { DataTable, type DataTableColumn } from "@/components/ui/DataTable";
import { DateRangeField } from "@/components/ui/DateField";
import { type DateRangeError, dateRangeError } from "@/components/ui/dateRange";
import { EmptyState } from "@/components/ui/EmptyState";
import { LoadingBar, SectionLoading } from "@/components/ui/Loading";
import { Notice } from "@/components/ui/Notice";
import { Select } from "@/components/ui/Select";
import { BOOKING_CANCELLER_LABELS, SPORT_LABELS, formatDateTime, formatDay } from "@/components/labels";

import { BookingLessonMenuFilter } from "./BookingLessonMenuFilter";
import { MAX_PERIOD_DAYS, type Period } from "./bookingPeriod";
import { SPORTS } from "./PendingVideoReviewList";

type Sport = Schemas["Sport"];
type BookingCanceller = Schemas["BookingCanceller"];

/** キャンセルした人の選択肢（契約の `BookingCanceller` の enum の順）。 */
export const BOOKING_CANCELLERS: BookingCanceller[] = ["student", "coach", "headquarters"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["CancelledBookingList"]; filter: Filter }
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
 * sport が空ならすべての競技、cancelledBy が空ならキャンセルした人を問わない。
 */
export type CancelledBookingFilter = {
  period: Period | null;
  coachId: string;
  lessonMenuId: string;
  sport: Sport | "";
  cancelledBy: BookingCanceller | "";
};
type Filter = CancelledBookingFilter;

const NO_NAME = "（名前を表示できません）";

/** キャンセルされた予約の一覧の列。 */
const CANCELLED_BOOKING_COLUMNS: DataTableColumn<Schemas["CancelledBooking"]>[] = [
  { header: "キャンセルした日時", cell: (booking) => formatDateTime(booking.cancelled_at) },
  { header: "枠の開始", cell: (booking) => formatDateTime(booking.starts_at) },
  { header: "コーチ", cell: (booking) => booking.coach_display_name ?? NO_NAME },
  { header: "生徒", cell: (booking) => booking.student_display_name ?? NO_NAME },
  {
    header: "キャンセルした人",
    cell: (booking) =>
      booking.cancelled_by === null ? "記録なし" : BOOKING_CANCELLER_LABELS[booking.cancelled_by],
  },
];

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間・コーチ・レッスンメニュー・スポーツ・キャンセルした人の指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。コーチ・レッスンメニュー・スポーツ・キャンセルした人は一覧から選び直してください。`;

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
 * キャンセルした人の選択欄（「すべて」と生徒・コーチ・本部。specs/ux/issue-992.yaml）は固定の選択肢なので読み込みを待たず、ほかの
 * 絞り込みと一緒に「この条件で見る」で API の `cancelled_by` に渡す。
 * `initialCoachId`・`initialLessonMenuId`・`initialSport`・`initialCancelledBy`（URL のクエリ）で開き、「この条件で見る」で読み直したら
 * `onFilterChange` で知らせる（URL に残すのは呼ぶ側）。
 */
export function CancelledBookingList({
  api,
  initialCoachId = "",
  initialLessonMenuId = "",
  initialSport = "",
  initialCancelledBy = "",
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listCancelledBookings" | "listCoaches" | "listLessonMenus">;
  initialCoachId?: string;
  initialLessonMenuId?: string;
  initialSport?: Sport | "";
  initialCancelledBy?: BookingCanceller | "";
  onFilterChange?: (filter: Filter) => void;
}) {
  // period が null なら API の既定の期間に任せる（開いたときの 1 回だけ）。
  const [draft, setDraft] = useState<Filter>({
    period: null,
    coachId: initialCoachId,
    lessonMenuId: initialLessonMenuId,
    sport: initialSport,
    cancelledBy: initialCancelledBy,
  });
  const [draftError, setDraftError] = useState<DateRangeError | null>(null);
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
        ...(filter.cancelledBy === "" ? {} : { cancelled_by: filter.cancelledBy }),
      });
      if (response.ok) return { kind: "loaded", list: response.data, filter };
      return {
        kind: "failed",
        message: response.code === "validation_failed" ? VALIDATION_FAILED : response.message,
      };
    },
    [api],
  );

  // 開いたときの読み込み（API の既定の期間と渡されたコーチ・レッスンメニュー・スポーツ・キャンセルした人）。読めたら実際に使った期間を
  // 入力欄に入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchList({
      period: null,
      coachId: initialCoachId,
      lessonMenuId: initialLessonMenuId,
      sport: initialSport,
      cancelledBy: initialCancelledBy,
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
  }, [fetchList, initialCoachId, initialLessonMenuId, initialSport, initialCancelledBy]);

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
    const error = dateRangeError(period, MAX_PERIOD_DAYS);
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
  // 開いたときの既定の期間を API から読んでいるあいだ（期間の欄を変えられなくし、画面の上に bar を出す）。
  const periodLoading = draft.period === null && state.kind === "loading";

  return (
    <div className="space-y-4">
      {periodLoading && <LoadingBar label="キャンセルされた予約を読み込んでいます" />}
      <p className="max-w-[40em] text-sm">
        すべてのコーチの枠への予約のうちキャンセルされたものを、キャンセルした日付（日本時間）・コーチ・レッスンメニュー・スポーツ・キャンセルした人で絞って、キャンセルの新しい順に見られます。期間は{" "}
        {MAX_PERIOD_DAYS} 日まで選べます。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submit(event)}>
        <DateRangeField
          value={period}
          onChange={(next) => setDraft({ ...draft, period: next })}
          helper={`${MAX_PERIOD_DAYS} 日まで選べます`}
          error={draftError}
          loading={periodLoading}
        />
        <Select
          label="コーチ"
          value={draft.coachId}
          loading={coaches.kind === "loading"}
          disabled={coaches.kind === "failed"}
          onChange={(event) => setDraft({ ...draft, coachId: event.target.value })}
        >
          <option value="">すべて</option>
          {coaches.kind === "loaded" &&
            coaches.coaches.map((coach) => (
              <option key={coach.coach_id} value={coach.coach_id}>
                {coach.display_name}
              </option>
            ))}
        </Select>
        <Select
          label="レッスンメニュー"
          value={draft.lessonMenuId}
          loading={lessonMenus.kind === "loading"}
          disabled={lessonMenus.kind === "failed"}
          onChange={(event) => setDraft({ ...draft, lessonMenuId: event.target.value })}
        >
          <option value="">すべて</option>
          {lessonMenus.kind === "loaded" &&
            lessonMenus.lessonMenus.map((menu) => (
              <option key={menu.lesson_menu_id} value={menu.lesson_menu_id}>
                {lessonMenuOptionLabel(menu)}
              </option>
            ))}
        </Select>
        <Select
          label="スポーツ"
          value={draft.sport}
          onChange={(event) => setDraft({ ...draft, sport: event.target.value as Sport | "" })}
        >
          <option value="">すべて</option>
          {SPORTS.map((sport) => (
            <option key={sport} value={sport}>
              {SPORT_LABELS[sport]}
            </option>
          ))}
        </Select>
        <Select
          label="キャンセルした人"
          value={draft.cancelledBy}
          onChange={(event) => setDraft({ ...draft, cancelledBy: event.target.value as BookingCanceller | "" })}
        >
          <option value="">すべて</option>
          {BOOKING_CANCELLERS.map((canceller) => (
            <option key={canceller} value={canceller}>
              {BOOKING_CANCELLER_LABELS[canceller]}
            </option>
          ))}
        </Select>
        <Button type="submit" loading={state.kind === "loading"}>
          この条件で見る
        </Button>
      </form>
      {coaches.kind === "failed" && (
        <Notice message={`コーチの一覧を読めませんでした。${coaches.message}`} status="danger" />
      )}
      {lessonMenus.kind === "failed" && (
        <Notice message={`レッスンメニューの一覧を読めませんでした。${lessonMenus.message}`} status="danger" />
      )}
      {appliedLessonMenuId !== "" && (
        <BookingLessonMenuFilter key={appliedLessonMenuId} api={api} lessonMenuId={appliedLessonMenuId} />
      )}
      {state.kind === "loading" && <SectionLoading message="キャンセルされた予約を読み込んでいます…" />}
      {state.kind === "failed" && <Notice message={state.message} status="danger" />}
      {state.kind === "loaded" && <LoadedBookings list={state.list} filter={state.filter} />}
    </div>
  );
}

/**
 * 0 件のときの文言。コーチ・レッスンメニュー・スポーツで絞っていれば、そのコーチの枠・そのメニュー・その競技のことだと言う。
 * キャンセルした人で絞っていれば、その人がキャンセルした予約がないことと、条件の変え方を添える。
 */
function emptyMessageOf({
  coachId,
  lessonMenuId,
  sport,
  cancelledBy,
}: Pick<Filter, "coachId" | "lessonMenuId" | "sport" | "cancelledBy">): string {
  const parts = [
    ...(sport === "" ? [] : [SPORT_LABELS[sport]]),
    ...(coachId === "" ? [] : ["このコーチの枠"]),
    ...(lessonMenuId === "" ? [] : ["このレッスンメニュー"]),
  ];
  const where = parts.length === 0 ? "" : `${parts.join("の")}で`;
  if (cancelledBy === "") return `この期間に${where}キャンセルされた予約はありません。`;
  return `この期間に${where}${BOOKING_CANCELLER_LABELS[cancelledBy]}がキャンセルした予約はありません。キャンセルした人を「すべて」にするか、期間を変えて見てください。`;
}

/**
 * 件数の行。期間に、絞ったキャンセルした人（「キャンセルした人: 本部」）と、スポーツ（「テニス 3 件」）を添える。0 件のときは件数を
 * 出さない（0 件の表示が文で伝える）。
 */
function countLineOf(list: Schemas["CancelledBookingList"], filter: Pick<Filter, "sport" | "cancelledBy">): string {
  const count = list.bookings.length;
  const sport = filter.sport === "" ? "" : SPORT_LABELS[filter.sport];
  return [
    `${formatDay(list.from)} 〜 ${formatDay(list.to)}`,
    ...(filter.cancelledBy === "" ? [] : [`キャンセルした人: ${BOOKING_CANCELLER_LABELS[filter.cancelledBy]}`]),
    ...(count === 0 ? (sport === "" ? [] : [sport]) : [sport === "" ? `${count} 件` : `${sport} ${count} 件`]),
  ].join("・");
}

function LoadedBookings({ list, filter }: { list: Schemas["CancelledBookingList"]; filter: Filter }) {
  return (
    <div className="space-y-2">
      <p role="status" className="text-sm">
        {countLineOf(list, filter)}
      </p>
      {list.bookings.length === 0 && <EmptyState variant="section" message={emptyMessageOf(filter)} />}
      {list.bookings.length > 0 && (
        <DataTable
          label="キャンセルされた予約の一覧"
          columns={CANCELLED_BOOKING_COLUMNS}
          rows={list.bookings}
          rowKey={(booking) => booking.booking_id}
        />
      )}
    </div>
  );
}
