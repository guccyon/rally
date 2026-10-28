"use client";

import { type FormEvent, useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime, SLOT_STATUS_LABELS, SPORT_LABELS } from "@/components/labels";

const SPORTS = Object.keys(SPORT_LABELS) as Schemas["Sport"][];

type CoachSlot = Schemas["CoachSlot"];

/** 定員の選択肢（契約の `capacity` は 1〜6。1 はマンツーマン）。 */
const CAPACITIES = [1, 2, 3, 4, 5, 6] as const;

function capacityLabel(capacity: number): string {
  return capacity === 1 ? "1 人（マンツーマン）" : `${capacity} 人`;
}

type ListState =
  | { kind: "loading" }
  | { kind: "loaded"; upcoming: CoachSlot[]; past: CoachSlot[] }
  | { kind: "failed"; message: string };

/**
 * API の並び（開始時刻の古い順）を、終わっていない枠（終了時刻が今より後）と終わった枠に分ける。
 * これからの枠は API の順のまま、終わった枠は新しい順（API の逆順）にする。
 */
function splitSlots(slots: CoachSlot[], now: number) {
  const upcoming = slots.filter((item) => new Date(item.slot.ends_at).getTime() > now);
  const past = slots.filter((item) => new Date(item.slot.ends_at).getTime() <= now).reverse();
  return { upcoming, past };
}

function SlotTable({ label, slots }: { label: string; slots: CoachSlot[] }) {
  return (
    <table aria-label={label} className="w-full text-left text-sm">
      <thead>
        <tr>
          <th className="py-1">スポーツ</th>
          <th className="py-1">日時</th>
          <th className="py-1">状態</th>
          <th className="py-1">予約</th>
        </tr>
      </thead>
      <tbody>
        {slots.map((item) => (
          <tr key={item.slot.slot_id} className="border-t">
            <td className="py-1">{SPORT_LABELS[item.slot.sport]}</td>
            <td className="py-1">
              {formatDateTime(item.slot.starts_at)}〜{formatDateTime(item.slot.ends_at)}
            </td>
            <td className="py-1">{SLOT_STATUS_LABELS[item.slot.status]}</td>
            <td className="py-1">
              {item.has_active_booking ? "予約あり" : "予約なし"}
              {/* 定員 1 の枠（マンツーマン）は今までどおり予約の有無だけを出す。 */}
              {item.slot.capacity > 1 && (
                <span className="block text-xs text-gray-600">
                  定員 {item.slot.capacity} 人・残り {item.slot.remaining_seats} 席
                </span>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** 枠の公開（publishSlot）と、開いたときから出す自分の枠の一覧（listMyCoachSlots）。公開したら一覧を取り直す。 */
export function PublishSlotForm({
  api,
}: {
  api: Pick<CoachApi, "publishSlot" | "listMyCoachSlots">;
}) {
  const [sport, setSport] = useState<Schemas["Sport"]>("tennis");
  const [startsAt, setStartsAt] = useState("");
  const [endsAt, setEndsAt] = useState("");
  const [capacity, setCapacity] = useState<number>(1);
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const fetchSlots = useCallback(async (): Promise<ListState> => {
    const response = await api.listMyCoachSlots();
    if (!response.ok) return { kind: "failed", message: response.message };
    return { kind: "loaded", ...splitSlots(response.data.slots, Date.now()) };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchSlots().then((next) => {
      if (active) setList(next);
    });
    return () => {
      active = false;
    };
  }, [fetchSlots]);

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
      capacity,
    });
    if (response.ok) {
      setResult({ ok: true, message: "枠を公開しました。" });
      setList(await fetchSlots());
    } else {
      setResult(response);
    }
    setBusy(false);
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
        <label className="flex flex-col">
          定員
          <select
            className="rounded border px-2 py-1"
            value={capacity}
            onChange={(event) => setCapacity(Number(event.target.value))}
          >
            {CAPACITIES.map((value) => (
              <option key={value} value={value}>
                {capacityLabel(value)}
              </option>
            ))}
          </select>
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
      {list.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {list.kind === "failed" && <ResultMessage result={{ ok: false, message: list.message }} />}
      {list.kind === "loaded" && list.upcoming.length === 0 && list.past.length === 0 && (
        <p className="text-sm">まだ枠を公開していません。</p>
      )}
      {list.kind === "loaded" && (list.upcoming.length > 0 || list.past.length > 0) && (
        <>
          <div className="space-y-2">
            <h3 className="font-semibold">これからの枠</h3>
            {list.upcoming.length === 0 ? (
              <p className="text-sm">これからの枠はありません。</p>
            ) : (
              <SlotTable label="これからの枠" slots={list.upcoming} />
            )}
          </div>
          {list.past.length > 0 && (
            <details>
              <summary className="cursor-pointer font-semibold">
                終わった枠（{list.past.length} 件）
              </summary>
              <SlotTable label="終わった枠" slots={list.past} />
            </details>
          )}
        </>
      )}
    </div>
  );
}
