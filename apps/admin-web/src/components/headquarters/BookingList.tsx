"use client";

import Link from "next/link";
import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { bookingLessonMenuLabel } from "@/components/coach/bookingLessonMenu";
import { formatDay, formatTime, toJstDate } from "@/components/labels";

import { bookingDetailPath } from "./BookingDetailHome";
import { BookingStatusBadge } from "./BookingStatusBadge";
import { type Period, defaultPeriod, isPendingSoon, periodError } from "./bookingPeriod";

type HeadquartersBooking = Schemas["HeadquartersBooking"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; period: Period; bookings: HeadquartersBooking[]; now: number }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";

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

/**
 * 本部の「予約」。すべてのコーチの枠への予約を、開始する日付（日本時間）の期間で出す（listBookings）。
 * 既定の期間は今日から 7 日。並びは契約の正本どおり返った順（開始時刻の古い順）のまま、日付ごとに分けて出す。
 * 各行の開始の時刻から、その予約の詳細へ移る。
 */
export function BookingList({ api }: { api: Pick<HeadquartersApi, "listBookings"> }) {
  const [draft, setDraft] = useState<Period>(() => defaultPeriod(Date.now()));
  const [draftError, setDraftError] = useState<string | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 期間を続けて変えたときに、前の期間の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);
  const initialPeriod = useRef(draft);

  const fetchBookings = useCallback(
    async (period: Period): Promise<State> => {
      const response = await api.listBookings(period);
      return response.ok
        ? {
            kind: "loaded",
            period: { from: response.data.from, to: response.data.to },
            bookings: response.data.bookings,
            now: Date.now(),
          }
        : { kind: "failed", message: response.message };
    },
    [api],
  );

  // 開いたときの読み込み（既定の期間）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchBookings(initialPeriod.current).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchBookings]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const error = periodError(draft);
    setDraftError(error);
    if (error !== null) return;
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchBookings(draft);
    if (request === latestRequest.current) setState(result);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        すべてのコーチの枠への予約を、開始する日付（日本時間）ごとに見られます。期間は 31 日まで選べます。
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
        <button type="submit" className="rounded bg-blue-600 px-4 py-1 text-white">
          この期間で見る
        </button>
      </form>
      {draftError !== null && <ResultMessage result={{ ok: false, message: draftError }} />}
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <LoadedBookings period={state.period} bookings={state.bookings} now={state.now} />
      )}
    </div>
  );
}

function LoadedBookings({
  period,
  bookings,
  now,
}: {
  period: Period;
  bookings: HeadquartersBooking[];
  now: number;
}) {
  const soonCount = bookings.filter((item) => isPendingSoon(item.booking, now)).length;
  return (
    <div className="space-y-4">
      <p className="text-sm">
        {formatDay(period.from)} 〜 {formatDay(period.to)}・{bookings.length} 件
      </p>
      {soonCount > 0 && (
        <p className="rounded border border-red-300 bg-red-50 p-2 text-sm text-red-700">
          確定待ちのまま 24 時間以内に始まる予約が {soonCount} 件あります。
        </p>
      )}
      {bookings.length === 0 && <p className="text-sm">この期間の予約はありません</p>}
      {groupByDay(bookings).map(({ day, items }) => (
        <div key={day}>
          <h3 className="mb-1 font-semibold">
            {formatDay(day)}（{items.length} 件）
          </h3>
          <table aria-label={`${formatDay(day)} の予約`} className="w-full text-left text-sm">
            <thead>
              <tr>
                <th className="py-1">開始</th>
                <th className="py-1">状態</th>
                <th className="py-1">コーチ</th>
                <th className="py-1">生徒</th>
                <th className="py-1">レッスンメニュー</th>
                <th className="py-1">種類</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => {
                const { booking } = item;
                const soon = isPendingSoon(booking, now);
                return (
                  <tr key={booking.booking_id} className={`border-t ${soon ? "bg-red-50" : ""}`}>
                    <td className="py-1">
                      <Link href={bookingDetailPath(booking.booking_id)} className="text-blue-600 underline">
                        {formatTime(booking.starts_at)}
                      </Link>
                    </td>
                    <td className="py-1">
                      <BookingStatusBadge status={booking.status} />
                      {soon && (
                        <span className="ml-2 rounded bg-red-600 px-2 text-xs text-white">
                          開始まで 24 時間以内
                        </span>
                      )}
                    </td>
                    <td className="py-1">{item.coach_display_name ?? NO_NAME}</td>
                    <td className="py-1">{item.student_display_name ?? NO_NAME}</td>
                    <td className="py-1">{bookingLessonMenuLabel(booking.lesson_menu)}</td>
                    <td className="py-1">{booking.is_trial ? "体験予約" : "—"}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ))}
    </div>
  );
}
