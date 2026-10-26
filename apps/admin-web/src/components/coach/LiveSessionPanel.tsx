"use client";

import { type FormEvent, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { NotImplemented } from "@/components/Section";
import { formatDateTime, SESSION_STATUS_LABELS } from "@/components/labels";

/** 通話への参加。セッションの開始・終了を呼ぶだけで、映像はモックなので出さない。 */
export function LiveSessionPanel({
  api,
}: {
  api: Pick<CoachApi, "startLiveLessonSession" | "endLiveLessonSession">;
}) {
  const [bookingId, setBookingId] = useState("");
  const [session, setSession] = useState<Schemas["LiveLessonSession"] | null>(null);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  async function onStart(event: FormEvent) {
    event.preventDefault();
    if (bookingId.trim() === "") {
      setResult({ ok: false, message: "予約 ID を入力してください。" });
      return;
    }
    setBusy(true);
    const response = await api.startLiveLessonSession({ booking_id: bookingId.trim() });
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
      <NotImplemented>
        自分の予約の一覧を取る API が admin-web の契約にないため、確定した予約の ID を入力します。
      </NotImplemented>
      <form onSubmit={onStart} className="flex flex-wrap items-end gap-3">
        <label className="flex grow flex-col">
          予約 ID
          <input
            className="rounded border px-2 py-1 font-mono"
            value={bookingId}
            onChange={(event) => setBookingId(event.target.value)}
          />
        </label>
        <button
          type="submit"
          disabled={busy || session?.status === "in_progress"}
          className="rounded bg-blue-600 px-4 py-1 text-white disabled:opacity-50"
        >
          通話を開始
        </button>
      </form>
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
