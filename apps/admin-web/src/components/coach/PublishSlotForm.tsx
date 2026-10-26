"use client";

import { type FormEvent, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { NotImplemented } from "@/components/Section";
import { formatDateTime, SLOT_STATUS_LABELS, SPORT_LABELS } from "@/components/labels";

const SPORTS = Object.keys(SPORT_LABELS) as Schemas["Sport"][];

/** 枠の公開（publishSlot）と、このログイン中に公開した枠の一覧。 */
export function PublishSlotForm({ api }: { api: Pick<CoachApi, "publishSlot"> }) {
  const [sport, setSport] = useState<Schemas["Sport"]>("tennis");
  const [startsAt, setStartsAt] = useState("");
  const [endsAt, setEndsAt] = useState("");
  const [slots, setSlots] = useState<Schemas["Slot"][]>([]);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (startsAt === "" || endsAt === "") {
      setResult({ ok: false, message: "開始と終了の日時を入力してください。" });
      return;
    }
    // datetime-local はブラウザのタイムゾーンの時刻。契約の date-time（RFC 3339）にする。
    const starts = new Date(startsAt);
    const ends = new Date(endsAt);
    if (starts >= ends) {
      setResult({ ok: false, message: "終了は開始より後にしてください。" });
      return;
    }
    setBusy(true);
    const response = await api.publishSlot({
      sport,
      starts_at: starts.toISOString(),
      ends_at: ends.toISOString(),
    });
    setBusy(false);
    if (response.ok) {
      setSlots((current) => [response.data, ...current]);
      setResult({ ok: true, message: "枠を公開しました。" });
    } else {
      setResult(response);
    }
  }

  return (
    <div className="space-y-4">
      <form onSubmit={onSubmit} className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col">
          スポーツ
          <select
            className="rounded border px-2 py-1"
            value={sport}
            onChange={(event) => setSport(event.target.value as Schemas["Sport"])}
          >
            {SPORTS.map((value) => (
              <option key={value} value={value}>
                {SPORT_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col">
          開始
          <input
            type="datetime-local"
            className="rounded border px-2 py-1"
            value={startsAt}
            onChange={(event) => setStartsAt(event.target.value)}
          />
        </label>
        <label className="flex flex-col">
          終了
          <input
            type="datetime-local"
            className="rounded border px-2 py-1"
            value={endsAt}
            onChange={(event) => setEndsAt(event.target.value)}
          />
        </label>
        <button
          type="submit"
          disabled={busy}
          className="rounded bg-blue-600 px-4 py-1 text-white disabled:opacity-50"
        >
          枠を公開
        </button>
      </form>
      <ResultMessage result={result} />
      <h3 className="font-medium">公開した枠</h3>
      <NotImplemented>
        自分の枠の一覧を取る API が admin-web の契約にないため、このログイン中に公開した枠だけを表示します。
      </NotImplemented>
      {slots.length === 0 ? (
        <p className="text-sm text-gray-500">まだ公開していません。</p>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left">
              <th>スポーツ</th>
              <th>開始</th>
              <th>終了</th>
              <th>状態</th>
            </tr>
          </thead>
          <tbody>
            {slots.map((slot) => (
              <tr key={slot.slot_id}>
                <td>{SPORT_LABELS[slot.sport]}</td>
                <td>{formatDateTime(slot.starts_at)}</td>
                <td>{formatDateTime(slot.ends_at)}</td>
                <td>{SLOT_STATUS_LABELS[slot.status]}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
