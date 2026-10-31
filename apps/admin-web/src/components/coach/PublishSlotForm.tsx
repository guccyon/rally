"use client";

import { type FormEvent, useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime, SLOT_STATUS_LABELS, SPORT_LABELS } from "@/components/labels";

import { addMinutesToLocal, selectableMenus } from "./slotLessonMenu";

const SPORTS = Object.keys(SPORT_LABELS) as Schemas["Sport"][];

type CoachSlot = Schemas["CoachSlot"];

/** 定員の選択肢（契約の `capacity` は 1〜6。1 はマンツーマン）。 */
const CAPACITIES = [1, 2, 3, 4, 5, 6] as const;

function capacityLabel(capacity: number): string {
  return capacity === 1 ? "1 人（マンツーマン）" : `${capacity} 人`;
}

type LessonMenu = Schemas["LessonMenu"];

type MenuState =
  | { kind: "loading" }
  | { kind: "loaded"; menus: LessonMenu[] }
  | { kind: "failed"; message: string };

// 画面ではスポーツ・公開中・同期で選択肢を絞り、終了をメニューの時間で決めてから送るので、メニューを付けて
// 送ったあとの 422（`validation_failed`）は、読み込んだ後にメニューが変わった（非公開にした・直した）ものと読む。
const MENU_CHANGED =
  "選んだレッスンメニューは、この枠に結びつけられませんでした。メニューが非公開になったか、スポーツや時間が変わった可能性があります。メニューを読み込み直したので、選び直してください。";

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
          <th className="py-1">メニュー</th>
          <th className="py-1">日時</th>
          <th className="py-1">状態</th>
          <th className="py-1">予約</th>
        </tr>
      </thead>
      <tbody>
        {slots.map((item) => (
          <tr key={item.slot.slot_id} className="border-t">
            <td className="py-1">{SPORT_LABELS[item.slot.sport]}</td>
            <td className="py-1">{item.slot.lesson_menu?.name ?? "—"}</td>
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

/**
 * 枠の公開（publishSlot）と、開いたときから出す自分の枠の一覧（listMyCoachSlots）。公開したら一覧を取り直す。
 * 枠には自分のレッスンメニュー（listMyLessonMenus）のうち、公開中の同期の、枠と同じスポーツのものを結びつけられる。
 * メニューを選んだら終了は開始＋メニューの時間で決まり、手では変えられない。
 */
export function PublishSlotForm({
  api,
}: {
  api: Pick<CoachApi, "publishSlot" | "listMyCoachSlots" | "listMyLessonMenus">;
}) {
  const [sport, setSport] = useState<Schemas["Sport"]>("tennis");
  const [startsAt, setStartsAt] = useState("");
  const [endsAt, setEndsAt] = useState("");
  const [capacity, setCapacity] = useState<number>(1);
  // 結びつけるメニューの ID。空文字列は「指定しない」。
  const [lessonMenuId, setLessonMenuId] = useState("");
  const [menus, setMenus] = useState<MenuState>({ kind: "loading" });
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const fetchSlots = useCallback(async (): Promise<ListState> => {
    const response = await api.listMyCoachSlots();
    if (!response.ok) return { kind: "failed", message: response.message };
    return { kind: "loaded", ...splitSlots(response.data.slots, Date.now()) };
  }, [api]);

  const fetchMenus = useCallback(async (): Promise<MenuState> => {
    const response = await api.listMyLessonMenus();
    if (!response.ok) return { kind: "failed", message: response.message };
    return { kind: "loaded", menus: response.data.lesson_menus };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchSlots().then((next) => {
      if (active) setList(next);
    });
    void fetchMenus().then((next) => {
      if (active) setMenus(next);
    });
    return () => {
      active = false;
    };
  }, [fetchSlots, fetchMenus]);

  const options = menus.kind === "loaded" ? selectableMenus(menus.menus, sport) : [];
  const selectedMenu = options.find((menu) => menu.lesson_menu_id === lessonMenuId);
  // メニューを選んでいれば、終了は開始＋メニューの時間（契約: メニューの時間と枠の長さが同じこと）。
  const effectiveEndsAt = selectedMenu
    ? addMinutesToLocal(startsAt, selectedMenu.duration_minutes)
    : endsAt;

  function changeSport(next: Schemas["Sport"]) {
    setSport(next);
    // メニューはスポーツが 1 つなので、スポーツを変えたら選び直してもらう。
    setLessonMenuId("");
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (lessonMenuId !== "" && !selectedMenu) {
      setResult({ ok: false, message: "選んだレッスンメニューはこの枠に結びつけられません。選び直してください。" });
      return;
    }
    if (startsAt === "" || effectiveEndsAt === "") {
      setResult({
        ok: false,
        message: selectedMenu ? "開始の日時を入力してください。" : "開始と終了の日時を入力してください。",
      });
      return;
    }
    // datetime-local はブラウザのタイムゾーンの時刻。契約の date-time（RFC 3339）にする。
    const starts = new Date(startsAt);
    const ends = new Date(effectiveEndsAt);
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
      ...(selectedMenu ? { lesson_menu_id: selectedMenu.lesson_menu_id } : {}),
    });
    if (response.ok) {
      setResult({ ok: true, message: "枠を公開しました。" });
      setList(await fetchSlots());
    } else if (selectedMenu && response.code === "validation_failed") {
      setResult({ ok: false, message: MENU_CHANGED });
      const next = await fetchMenus();
      setMenus(next);
      const stillSelectable =
        next.kind === "loaded" &&
        selectableMenus(next.menus, sport).some(
          (menu) => menu.lesson_menu_id === selectedMenu.lesson_menu_id,
        );
      if (!stillSelectable) setLessonMenuId("");
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
            onChange={(event) => changeSport(event.target.value as Schemas["Sport"])}
          >
            {SPORTS.map((value) => (
              <option key={value} value={value}>
                {SPORT_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col">
          レッスンメニュー
          <select
            className="rounded border px-2 py-1"
            value={selectedMenu ? lessonMenuId : ""}
            disabled={menus.kind !== "loaded" || options.length === 0}
            aria-describedby="slot-lesson-menu-note"
            onChange={(event) => setLessonMenuId(event.target.value)}
          >
            <option value="">指定しない</option>
            {options.map((menu) => (
              <option key={menu.lesson_menu_id} value={menu.lesson_menu_id}>
                {menu.name}（{menu.duration_minutes} 分）
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
            value={effectiveEndsAt}
            readOnly={selectedMenu !== undefined}
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
      <p id="slot-lesson-menu-note" className="text-sm text-gray-600">
        {menus.kind === "loading" && "レッスンメニューを読み込み中…"}
        {menus.kind === "failed" &&
          `レッスンメニューを読み込めませんでした（${menus.message}）。メニューを指定せずに公開できます。`}
        {menus.kind === "loaded" &&
          (selectedMenu
            ? `終了は開始からメニューの時間（${selectedMenu.duration_minutes} 分）で決まります。`
            : options.length === 0
              ? `${SPORT_LABELS[sport]}の公開中の同期のレッスンメニューはありません。メニューを指定せずに公開できます。`
              : "レッスンメニューを選ぶと、終了はメニューの時間で決まります。")}
      </p>
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
