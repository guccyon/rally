"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { BOOKING_CANCELLER_LABELS, formatDateTime, formatDay } from "@/components/labels";

import { MAX_PERIOD_DAYS, type Period, periodError } from "./bookingPeriod";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; list: Schemas["CancelledBookingList"] }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間の指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。`;

/**
 * 本部の「キャンセルされた予約」。すべてのコーチの枠への予約のうちキャンセルされたものを、キャンセルした日付（日本時間）の
 * 期間で出す（listCancelledBookings）。開いたときは期間を付けずに読み、API の既定（今日までの 7 日）で実際に使った期間を
 * 入力欄に入れる。並び（キャンセルの新しい順）は契約の正本どおり API が済ませているので、返った順と値のまま出す。
 */
export function CancelledBookingList({ api }: { api: Pick<HeadquartersApi, "listCancelledBookings"> }) {
  // null なら API の既定の期間に任せる（開いたときの 1 回だけ）。
  const [draft, setDraft] = useState<Period | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 期間を続けて変えたときに、前の期間の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchList = useCallback(
    async (period: Period | null): Promise<State> => {
      const response = await api.listCancelledBookings(period ?? {});
      if (response.ok) return { kind: "loaded", list: response.data };
      return {
        kind: "failed",
        message: response.code === "validation_failed" ? VALIDATION_FAILED : response.message,
      };
    },
    [api],
  );

  // 開いたときの読み込み（API の既定の期間）。読めたら実際に使った期間を入力欄に入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchList(null).then((next) => {
      if (!active || request !== latestRequest.current) return;
      setState(next);
      if (next.kind === "loaded") {
        const used = { from: next.list.from, to: next.list.to };
        setDraft((current) => current ?? used);
      }
    });
    return () => {
      active = false;
    };
  }, [fetchList]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    const period = draft ?? { from: "", to: "" };
    const error = periodError(period);
    setDraftError(error);
    if (error !== null) return;
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchList(period);
    if (request === latestRequest.current) setState(result);
  }

  const period = draft ?? { from: "", to: "" };

  return (
    <div className="space-y-4">
      <p className="text-sm">
        すべてのコーチの枠への予約のうちキャンセルされたものを、キャンセルした日付（日本時間）で絞って、キャンセルの新しい順に見られます。期間は{" "}
        {MAX_PERIOD_DAYS} 日まで選べます。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submit(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded border px-2 py-1"
            value={period.from}
            onChange={(event) => setDraft({ ...period, from: event.target.value })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded border px-2 py-1"
            value={period.to}
            onChange={(event) => setDraft({ ...period, to: event.target.value })}
          />
        </label>
        <button type="submit" className="rounded bg-blue-600 px-4 py-1 text-white">
          この期間で見る
        </button>
      </form>
      {draftError !== null && <ResultMessage result={{ ok: false, message: draftError }} />}
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && <LoadedBookings list={state.list} />}
    </div>
  );
}

function LoadedBookings({ list }: { list: Schemas["CancelledBookingList"] }) {
  return (
    <div className="space-y-2">
      <p className="text-sm">
        {formatDay(list.from)} 〜 {formatDay(list.to)}・{list.bookings.length} 件
      </p>
      {list.bookings.length === 0 && <p className="text-sm">この期間にキャンセルされた予約はありません</p>}
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
