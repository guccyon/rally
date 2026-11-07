"use client";

import Link from "next/link";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { DataTable, type DataTableColumn } from "@/components/ui/DataTable";
import { Notice } from "@/components/ui/Notice";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { bookingLessonMenuLabel } from "@/components/coach/bookingLessonMenu";
import { SPORT_LABELS, formatDay, formatTime, toJstDate } from "@/components/labels";

import { bookingDetailPath } from "./BookingDetailHome";
import { BookingStatusBadge } from "./BookingStatusBadge";
import { type Period, defaultPeriod, isPendingSoon, periodError } from "./bookingPeriod";
import { SPORTS } from "./PendingVideoReviewList";
import { Button } from "@/components/ui/Button";

type HeadquartersBooking = Schemas["HeadquartersBooking"];
type Sport = Schemas["Sport"];

type State =
  | { kind: "loading" }
  | {
      kind: "loaded";
      period: Period;
      trialOnly: boolean;
      sport: Sport | "";
      bookings: HeadquartersBooking[];
      now: number;
    }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";

// レッスンメニューで絞っているときの 422 の理由（期間は送る前に確かめるので、残るのは URL の ID が UUID でないとき）。
// サーバーの `message` は表示用ではない（契約）ので、直し方をここで伝える。
const LESSON_MENU_VALIDATION_FAILED =
  "絞り込みの指定が正しくありません。レッスンメニューの一覧の「直近 30 日の予約」から開き直してください。";

/** 開始する日付（日本時間）ごとにまとめる。API の順（開始時刻の古い順）なので、日付の並びも古い順になる。 */
function groupByDay(bookings: HeadquartersBooking[]): { day: string; items: HeadquartersBooking[] }[] {
  const groups: { day: string; items: HeadquartersBooking[] }[] = [];
  for (const item of bookings) {
    const day = toJstDate(item.booking.starts_at);
    const last = groups.at(-1);
    if (last?.day === day) last.items.push(item);
    else groups.push({ day, items: [item] });
  }
  return groups;
}

/** 一覧の上の説明の主語（何の予約を見ているか）。絞り込みはコーチ・生徒・レッスンメニューの順に「、」でつなぐ。 */
function subjectOf(
  coachId: string | undefined,
  studentId: string | undefined,
  lessonMenuId: string | undefined,
): string {
  const parts = [
    ...(coachId === undefined ? [] : ["このコーチの枠への"]),
    ...(studentId === undefined ? [] : ["この生徒の"]),
    ...(lessonMenuId === undefined ? [] : ["このレッスンメニューの"]),
  ];
  return parts.length === 0 ? "すべてのコーチの枠への予約" : `${parts.join("、")}予約`;
}

/** 0 件のときの文言。体験予約だけか・レッスンメニューで絞っているか・スポーツで絞っているかで言い分ける。 */
function emptyMessage(trialOnly: boolean, byLessonMenu: boolean, sport: Sport | ""): string {
  const kind = trialOnly ? "体験予約" : "予約";
  const parts = [
    ...(byLessonMenu ? ["このレッスンメニューの"] : []),
    ...(sport === "" ? [] : [`${SPORT_LABELS[sport]}の`]),
  ];
  return parts.length === 0 ? `この期間の${kind}はありません` : `この期間に、${parts.join("、")}${kind}はありません`;
}

/**
 * 一覧の条件: 期間と、体験予約だけに絞るか（絞るときだけ契約の `trial=true` を付ける）と、スポーツ（空なら絞らない。
 * 選んだときだけ契約の `sport` を付ける）。
 */
type Filter = Period & { trialOnly: boolean; sport: Sport | "" };

/**
 * 本部の「予約」。すべてのコーチの枠への予約を、開始する日付（日本時間）の期間で出す（listBookings）。
 * 既定の期間は今日から 7 日。並びは契約の正本どおり返った順（開始時刻の古い順）のまま、日付ごとに分けて出す。
 * `coachId` があれば、そのコーチの枠への予約だけに絞る（`coach_id`）。`studentId` があれば、その生徒が予約した予約だけに
 * 絞る（`student_id`）。`lessonMenuId` があれば、予約した時点のレッスンメニューがそのメニューの予約だけに絞る（`lesson_menu_id`）。
 * 「スポーツ」を選べば、その競技の予約だけに絞る（`sport`。予約の競技は枠のコーチの提供スポーツで決まる。契約）。
 * どれも組み合わせられ、あるものすべてで絞る。期間を変えても絞り込みは残る。
 * レッスンメニューで絞っていて 422（ID が UUID でない）なら、開き直す場所を出す。
 * 各行の開始の時刻から、その予約の詳細へ移る。
 * 「体験予約だけ」・「スポーツ」を変えると、選んでいる期間でそのまま読み直す（期間は「この期間で見る」で読み直す。
 * ほかの競技の絞り込みの画面と同じく、スポーツは選んだときに読み直す）。
 */
export function BookingList({
  api,
  coachId,
  studentId,
  lessonMenuId,
}: {
  api: Pick<HeadquartersApi, "listBookings">;
  coachId?: string;
  studentId?: string;
  lessonMenuId?: string;
}) {
  const [draft, setDraft] = useState<Period>(() => defaultPeriod(Date.now()));
  const [trialOnly, setTrialOnly] = useState(false);
  const [sport, setSport] = useState<Sport | "">("");
  const [draftError, setDraftError] = useState<string | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 期間を続けて変えたときに、前の期間の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);
  const initialPeriod = useRef(draft);

  const fetchBookings = useCallback(
    async ({ trialOnly, sport, ...period }: Filter): Promise<State> => {
      const response = await api.listBookings({
        ...period,
        ...(coachId === undefined ? {} : { coach_id: coachId }),
        ...(studentId === undefined ? {} : { student_id: studentId }),
        ...(trialOnly ? { trial: true } : {}),
        ...(lessonMenuId === undefined ? {} : { lesson_menu_id: lessonMenuId }),
        ...(sport === "" ? {} : { sport }),
      });
      return response.ok
        ? {
            kind: "loaded",
            period: { from: response.data.from, to: response.data.to },
            trialOnly,
            sport,
            bookings: response.data.bookings,
            now: Date.now(),
          }
        : {
            kind: "failed",
            message:
              lessonMenuId !== undefined && response.code === "validation_failed"
                ? LESSON_MENU_VALIDATION_FAILED
                : response.message,
          };
    },
    [api, coachId, studentId, lessonMenuId],
  );

  // 開いたときの読み込み（既定の期間）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchBookings({ ...initialPeriod.current, trialOnly: false, sport: "" }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchBookings]);

  async function load(filter: Filter) {
    const error = periodError(filter);
    setDraftError(error);
    if (error !== null) return;
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchBookings(filter);
    if (request === latestRequest.current) setState(result);
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    await load({ ...draft, trialOnly, sport });
  }

  async function changeTrialOnly(next: boolean) {
    setTrialOnly(next);
    await load({ ...draft, trialOnly: next, sport });
  }

  async function changeSport(next: Sport | "") {
    setSport(next);
    await load({ ...draft, trialOnly, sport: next });
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        {`${subjectOf(coachId, studentId, lessonMenuId)}を、開始する日付（日本時間）ごとに見られます。期間は 31 日まで選べます。`}
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
        <Button type="submit" variant="secondary">
          この期間で見る
        </Button>
        <label className="flex items-center gap-1 py-1">
          <input
            type="checkbox"
            checked={trialOnly}
            onChange={(event) => void changeTrialOnly(event.target.checked)}
          />
          体験予約だけ
        </label>
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={sport}
            onChange={(event) => void changeSport(event.target.value as Sport | "")}
          >
            <option value="">すべて</option>
            {SPORTS.map((value) => (
              <option key={value} value={value}>
                {SPORT_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
      </form>
      {draftError !== null && <ResultMessage result={{ ok: false, message: draftError }} />}
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <LoadedBookings
          period={state.period}
          trialOnly={state.trialOnly}
          sport={state.sport}
          byLessonMenu={lessonMenuId !== undefined}
          bookings={state.bookings}
          now={state.now}
        />
      )}
    </div>
  );
}

/** 日ごとの予約の表の列。確定待ちのまま 24 時間以内に始まる予約は、状態のセルに文字のしるしを添える（行の面は highlightRow）。 */
function bookingColumns(now: number): DataTableColumn<HeadquartersBooking>[] {
  return [
    {
      header: "開始",
      cell: ({ booking }) => (
        <Link href={bookingDetailPath(booking.booking_id)} className="text-primary underline">
          {formatTime(booking.starts_at)}
        </Link>
      ),
    },
    {
      header: "状態",
      cell: ({ booking }) => (
        <>
          <BookingStatusBadge status={booking.status} />
          {isPendingSoon(booking, now) && (
            <span className="ml-2 rounded-sm bg-danger px-2 text-xs text-on-primary">開始まで 24 時間以内</span>
          )}
        </>
      ),
    },
    { header: "コーチ", cell: (item) => item.coach_display_name ?? NO_NAME },
    { header: "生徒", cell: (item) => item.student_display_name ?? NO_NAME },
    { header: "レッスンメニュー", cell: ({ booking }) => bookingLessonMenuLabel(booking.lesson_menu) },
    { header: "種類", cell: ({ booking }) => (booking.is_trial ? "体験予約" : "—") },
  ];
}

function LoadedBookings({
  period,
  trialOnly,
  sport,
  byLessonMenu,
  bookings,
  now,
}: {
  period: Period;
  trialOnly: boolean;
  sport: Sport | "";
  byLessonMenu: boolean;
  bookings: HeadquartersBooking[];
  now: number;
}) {
  const soonCount = bookings.filter((item) => isPendingSoon(item.booking, now)).length;
  return (
    <div className="space-y-4">
      <p className="text-sm">
        {formatDay(period.from)} 〜 {formatDay(period.to)}・{sport === "" ? "" : `${SPORT_LABELS[sport]} `}
        {trialOnly ? "体験予約 " : ""}
        {bookings.length} 件
      </p>
      {soonCount > 0 && (
        <Notice message={`確定待ちのまま 24 時間以内に始まる予約が ${soonCount} 件あります。`} status="warning" />
      )}
      {bookings.length === 0 && <p className="text-sm">{emptyMessage(trialOnly, byLessonMenu, sport)}</p>}
      {groupByDay(bookings).map(({ day, items }) => (
        <div key={day}>
          <h3 className="mb-1 font-semibold">
            {formatDay(day)}（{items.length} 件）
          </h3>
          <DataTable
            label={`${formatDay(day)} の予約`}
            columns={bookingColumns(now)}
            rows={items}
            rowKey={(item) => item.booking.booking_id}
            highlightRow={(item) => isPendingSoon(item.booking, now)}
          />
        </div>
      ))}
    </div>
  );
}
