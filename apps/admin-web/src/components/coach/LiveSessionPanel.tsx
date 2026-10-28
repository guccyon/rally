"use client";

import { type ReactNode, useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import {
  BOOKING_STATUS_LABELS,
  formatDateTime,
  SESSION_STATUS_LABELS,
} from "@/components/labels";

type CoachBooking = Schemas["CoachBooking"];

type ListState =
  | { kind: "loading" }
  | { kind: "loaded"; upcoming: CoachBooking[]; past: CoachBooking[] }
  | { kind: "failed"; message: string };

/**
 * API の並び（開始時刻の古い順）を、終わっていない予約（終了時刻が今より後）と終わった予約に分ける。
 * これからの予約は API の順のまま、終わった予約は新しい順（API の逆順）にする。
 */
function splitBookings(bookings: CoachBooking[], now: number) {
  const upcoming = bookings.filter((item) => new Date(item.slot.ends_at).getTime() > now);
  const past = bookings.filter((item) => new Date(item.slot.ends_at).getTime() <= now).reverse();
  return { upcoming, past };
}

function BookingTable({
  label,
  bookings,
  renderAction,
}: {
  label: string;
  bookings: CoachBooking[];
  renderAction?: (item: CoachBooking) => ReactNode;
}) {
  return (
    <table aria-label={label} className="w-full text-left text-sm">
      <thead>
        <tr>
          <th className="py-1">日時</th>
          <th className="py-1">生徒</th>
          <th className="py-1">状態</th>
          {renderAction && <th className="py-1">操作</th>}
        </tr>
      </thead>
      <tbody>
        {bookings.map((item) => (
          <tr key={item.booking.booking_id} className="border-t">
            <td className="py-1">
              {formatDateTime(item.slot.starts_at)}〜{formatDateTime(item.slot.ends_at)}
            </td>
            <td className="py-1">{item.student_display_name ?? "（名前を表示できません）"}</td>
            <td className="py-1">{BOOKING_STATUS_LABELS[item.booking.status]}</td>
            {renderAction && <td className="py-1">{renderAction(item)}</td>}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/**
 * 通話への参加。開いたときに自分の予約の一覧（listMyCoachBookings）を出し、確定した予約の行から
 * セッションを開始し、同じセッションを終了する。映像はモックなので出さない。
 */
export function LiveSessionPanel({
  api,
}: {
  api: Pick<CoachApi, "listMyCoachBookings" | "startLiveLessonSession" | "endLiveLessonSession">;
}) {
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [session, setSession] = useState<Schemas["LiveLessonSession"] | null>(null);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const fetchBookings = useCallback(async (): Promise<ListState> => {
    const response = await api.listMyCoachBookings();
    if (!response.ok) return { kind: "failed", message: response.message };
    return { kind: "loaded", ...splitBookings(response.data.bookings, Date.now()) };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchBookings().then((next) => {
      if (active) setList(next);
    });
    return () => {
      active = false;
    };
  }, [fetchBookings]);

  async function onStart(bookingId: string) {
    setBusy(true);
    const response = await api.startLiveLessonSession({ booking_id: bookingId });
    setBusy(false);
    if (response.ok) {
      setSession(response.data);
      setResult({ ok: true, message: "通話を開始しました。" });
    } else {
      setResult(response);
    }
  }

  async function onEnd() {
    if (!session) return;
    setBusy(true);
    const response = await api.endLiveLessonSession(session.session_id);
    setBusy(false);
    if (response.ok) {
      setSession(response.data);
      setResult({ ok: true, message: "通話を終了しました。" });
    } else {
      setResult(response);
    }
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        あなたの枠に入った予約です。確定した予約の「通話を開始」を押すと、その生徒との通話を始めます。
      </p>
      {list.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {list.kind === "failed" && <ResultMessage result={{ ok: false, message: list.message }} />}
      {list.kind === "loaded" && list.upcoming.length === 0 && list.past.length === 0 && (
        <p className="text-sm">まだ予約はありません。</p>
      )}
      {list.kind === "loaded" && (list.upcoming.length > 0 || list.past.length > 0) && (
        <>
          <div className="space-y-2">
            <h3 className="font-semibold">これからの予約</h3>
            {list.upcoming.length === 0 ? (
              <p className="text-sm">これからの予約はありません。</p>
            ) : (
              <BookingTable
                label="これからの予約"
                bookings={list.upcoming}
                renderAction={(item) =>
                  item.booking.status === "confirmed" && (
                    <button
                      type="button"
                      disabled={busy || session?.status === "in_progress"}
                      onClick={() => void onStart(item.booking.booking_id)}
                      className="rounded bg-blue-600 px-3 py-1 text-white disabled:opacity-50"
                    >
                      通話を開始
                    </button>
                  )
                }
              />
            )}
          </div>
          {list.past.length > 0 && (
            <details>
              <summary className="cursor-pointer font-semibold">
                終わった予約（{list.past.length} 件）
              </summary>
              <BookingTable label="終わった予約" bookings={list.past} />
            </details>
          )}
        </>
      )}
      {session && (
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 text-sm">
          <dt>状態</dt>
          <dd>{SESSION_STATUS_LABELS[session.status]}</dd>
          <dt>セッション ID</dt>
          <dd className="font-mono">{session.session_id}</dd>
          <dt>開始</dt>
          <dd>{formatDateTime(session.started_at)}</dd>
          {session.ended_at && (
            <>
              <dt>終了</dt>
              <dd>{formatDateTime(session.ended_at)}</dd>
            </>
          )}
          {session.duration_sec != null && (
            <>
              <dt>通話時間</dt>
              <dd>{session.duration_sec} 秒</dd>
            </>
          )}
        </dl>
      )}
      {session?.status === "in_progress" && (
        <>
          <p className="text-sm text-gray-500">映像はモックのため表示しません。</p>
          <button
            type="button"
            disabled={busy}
            onClick={onEnd}
            className="rounded bg-red-600 px-4 py-1 text-white disabled:opacity-50"
          >
            通話を終了
          </button>
        </>
      )}
      <ResultMessage result={result} />
    </div>
  );
}
