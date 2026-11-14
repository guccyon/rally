"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { Button } from "@/components/ui/Button";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { StatusTag } from "@/components/ui/StatusTag";
import { SLOT_STATE_STATUS } from "@/components/ui/status";
import { MAX_PERIOD_DAYS, type Period, periodError } from "@/components/headquarters/bookingPeriod";
import { formatDateTime, formatDay, SLOT_STATUS_LABELS, SPORT_LABELS } from "@/components/labels";

import { addMinutesToLocal, selectableMenus } from "./slotLessonMenu";
import { WithdrawSlotAction } from "./WithdrawSlotAction";

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

// 開始が公開した時刻以前の 422（`validation_failed`）。サーバーの `message` は表示用ではない（契約）ので、直し方をここで伝える。
const STARTS_IN_PAST =
  "開始の日時が過去（または今）のため、枠を公開できませんでした。今より後の日時を選んで、もう一度公開してください。";

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間の指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。`;

const NO_PERIOD: Period = { from: "", to: "" };

type ListState =
  | { kind: "loading" }
  | {
      kind: "loaded";
      upcoming: CoachSlot[];
      past: CoachSlot[];
      fetchedAt: number;
      /** 読んだ期間（枠の開始の日付。日本時間）。null は期間で絞らずに読んだとき。 */
      period: Period | null;
    }
  | { kind: "failed"; message: string };

/** 枠が 0 件のときの文言。期間で絞っていれば、その期間を言う。 */
function emptyMessageOf(period: Period | null): string {
  return period === null
    ? "まだ枠を公開していません。"
    : `${formatDay(period.from)} 〜 ${formatDay(period.to)}に始まる枠はありません。`;
}

/**
 * API の並び（開始時刻の古い順）を、終わっていない枠（終了時刻が今より後）と終わった枠に分ける。
 * これからの枠は API の順のまま、終わった枠は新しい順（API の逆順）にする。
 */
function splitSlots(slots: CoachSlot[], now: number) {
  const upcoming = slots.filter((item) => new Date(item.slot.ends_at).getTime() > now);
  const past = slots.filter((item) => new Date(item.slot.ends_at).getTime() <= now).reverse();
  return { upcoming, past };
}

/**
 * 取り下げ（withdrawMySlot）を出す枠か。契約が取り下げを認める、公開中の・有効な予約がない・まだ始まっていない枠だけ。
 * 予約が入っている枠（予約済みも）には出さない。始まったかは一覧を読んだ時刻で見る。
 */
function canWithdraw(item: CoachSlot, now: number): boolean {
  return (
    item.slot.status === "open" &&
    !item.has_active_booking &&
    new Date(item.slot.starts_at).getTime() > now
  );
}

function SlotTable({
  label,
  slots,
  withdraw,
}: {
  label: string;
  slots: CoachSlot[];
  /** これからの枠だけに渡す。渡したら取り下げの列を出す。 */
  withdraw?: { now: number; onWithdraw: (item: CoachSlot) => Promise<void> };
}) {
  return (
    <table aria-label={label} className="w-full text-left text-sm">
      <thead>
        <tr>
          <th className="py-1">スポーツ</th>
          <th className="py-1">メニュー</th>
          <th className="py-1">日時</th>
          <th className="py-1">状態</th>
          <th className="py-1">予約</th>
          {withdraw && <th className="py-1">取り下げ</th>}
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
            <td className="py-1">
              <StatusTag label={SLOT_STATUS_LABELS[item.slot.status]} status={SLOT_STATE_STATUS[item.slot.status]} />
            </td>
            <td className="py-1">
              {item.has_active_booking ? "予約あり" : "予約なし"}
              {/* 定員 1 の枠（マンツーマン）は今までどおり予約の有無だけを出す。 */}
              {item.slot.capacity > 1 && (
                <span className="block text-xs text-text-muted">
                  定員 {item.slot.capacity} 人・残り {item.slot.remaining_seats} 席
                </span>
              )}
            </td>
            {withdraw && (
              <td className="py-1">
                {canWithdraw(item, withdraw.now) && (
                  <WithdrawSlotAction onWithdraw={() => withdraw.onWithdraw(item)} />
                )}
              </td>
            )}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/**
 * 枠の公開（publishSlot）と、開いたときから出す自分の枠の一覧（listMyCoachSlots）。公開したら一覧を取り直す。
 * これからの枠のうち予約の入っていない公開中の枠は、行の中で確かめてから取り下げられる（withdrawMySlot）。
 * 枠には自分のレッスンメニュー（listMyLessonMenus）のうち、公開中の同期の、枠と同じスポーツのものを結びつけられる。
 * メニューを選んだら終了は開始＋メニューの時間で決まり、手では変えられない。
 * 一覧は「開始日」「終了日」で枠の開始の日付（日本時間）の期間に絞れる（API の `from`・`to`）。本部の画面の期間の欄と同じく、
 * 送る前に確かめて「この期間で見る」で読み直し、「期間を外す」ですべての枠に戻す。開いたときは今までどおり期間を付けずに読み、
 * 公開・取り下げのあとの取り直しは確定した期間で読む。
 */
export function PublishSlotForm({
  api,
}: {
  api: Pick<CoachApi, "publishSlot" | "listMyCoachSlots" | "listMyLessonMenus" | "withdrawMySlot">;
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
  // 期間の欄の値（「この期間で見る」を押すまでは一覧に効かない）と、確定した期間（公開・取り下げのあとの取り直しにも使う）。
  const [draftPeriod, setDraftPeriod] = useState<Period>(NO_PERIOD);
  const [periodDraftError, setPeriodDraftError] = useState<string | null>(null);
  const [appliedPeriod, setAppliedPeriod] = useState<Period | null>(null);
  const appliedPeriodRef = useRef<Period | null>(null);
  // 期間を続けて変えたときに、前の期間の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchSlots = useCallback(
    async (period: Period | null): Promise<ListState> => {
      const response = await api.listMyCoachSlots(period ?? {});
      if (!response.ok) {
        const message = period !== null && response.code === "validation_failed" ? VALIDATION_FAILED : response.message;
        return { kind: "failed", message };
      }
      const now = Date.now();
      return { kind: "loaded", ...splitSlots(response.data.slots, now), fetchedAt: now, period };
    },
    [api],
  );

  /** 確定した期間で一覧を読み直す（後から読み直したときは、前の遅れた結果を捨てる）。 */
  const reloadSlots = useCallback(async () => {
    const request = ++latestRequest.current;
    const next = await fetchSlots(appliedPeriodRef.current);
    if (request === latestRequest.current) setList(next);
  }, [fetchSlots]);

  const fetchMenus = useCallback(async (): Promise<MenuState> => {
    const response = await api.listMyLessonMenus();
    if (!response.ok) return { kind: "failed", message: response.message };
    return { kind: "loaded", menus: response.data.lesson_menus };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchSlots(null).then((next) => {
      if (active && request === latestRequest.current) setList(next);
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
      await reloadSlots();
    } else if (response.code === "validation_failed" && starts.getTime() <= Date.now() + (response.clockOffsetMs ?? 0)) {
      // 開始が過去なら、メニューの有無にかかわらず 422 の理由は開始（端末の時計はサーバーとのずれで直す）。
      setResult({ ok: false, message: STARTS_IN_PAST });
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

  async function withdrawSlot(item: CoachSlot) {
    const response = await api.withdrawMySlot(item.slot.slot_id);
    if (response.ok) {
      setResult({ ok: true, message: "枠を取り下げました。" });
      // 応答の Slot には lesson_menu が載らない（契約）ので、行の状態だけを応答で置き換える。
      setList((current) =>
        current.kind === "loaded"
          ? {
              ...current,
              upcoming: current.upcoming.map((row) =>
                row.slot.slot_id === item.slot.slot_id
                  ? { ...row, slot: { ...row.slot, status: response.data.status } }
                  : row,
              ),
            }
          : current,
      );
      return;
    }
    // 409（予約が入った・始まった・すでに閉じた）などはサーバーの ErrorCode の文言を出し、一覧を今の状態に取り直す。
    setResult(response);
    if (response.code !== undefined) await reloadSlots();
  }

  async function applyPeriod(period: Period | null) {
    appliedPeriodRef.current = period;
    setAppliedPeriod(period);
    setList({ kind: "loading" });
    await reloadSlots();
  }

  async function submitPeriod(event: FormEvent) {
    event.preventDefault();
    const error = periodError(draftPeriod);
    setPeriodDraftError(error);
    if (error !== null) return;
    await applyPeriod(draftPeriod);
  }

  async function clearPeriod() {
    setDraftPeriod(NO_PERIOD);
    setPeriodDraftError(null);
    await applyPeriod(null);
  }

  return (
    <div className="space-y-4">
      <form onSubmit={onSubmit} className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col">
          スポーツ
          <select
            className="rounded-md border px-2 py-1"
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
            className="rounded-md border px-2 py-1"
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
            className="rounded-md border px-2 py-1"
            value={startsAt}
            onChange={(event) => setStartsAt(event.target.value)}
          />
        </label>
        <label className="flex flex-col">
          終了
          <input
            type="datetime-local"
            className="rounded-md border px-2 py-1"
            value={effectiveEndsAt}
            readOnly={selectedMenu !== undefined}
            onChange={(event) => setEndsAt(event.target.value)}
          />
        </label>
        <label className="flex flex-col">
          定員
          <select
            className="rounded-md border px-2 py-1"
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
        <Button type="submit" disabled={busy}>
          枠を公開
        </Button>
      </form>
      <p id="slot-lesson-menu-note" className="text-sm text-text-muted">
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
      <p className="text-sm">
        期間を選ぶと、一覧をその期間（日本時間）に始まる枠だけにします（期間は {MAX_PERIOD_DAYS} 日まで）。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submitPeriod(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={draftPeriod.from}
            onChange={(event) => setDraftPeriod({ ...draftPeriod, from: event.target.value })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={draftPeriod.to}
            onChange={(event) => setDraftPeriod({ ...draftPeriod, to: event.target.value })}
          />
        </label>
        <Button variant="secondary" type="submit">
          この期間で見る
        </Button>
        {appliedPeriod !== null && (
          <Button variant="secondary" onClick={() => void clearPeriod()}>
            期間を外す
          </Button>
        )}
      </form>
      {periodDraftError !== null && <ResultMessage result={{ ok: false, message: periodDraftError }} />}
      {list.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {list.kind === "failed" && <ResultMessage result={{ ok: false, message: list.message }} />}
      {list.kind === "loaded" && list.upcoming.length === 0 && list.past.length === 0 && (
        <p className="text-sm">{emptyMessageOf(list.period)}</p>
      )}
      {list.kind === "loaded" && (list.upcoming.length > 0 || list.past.length > 0) && (
        <>
          <div className="space-y-2">
            <h3 className="font-semibold">これからの枠</h3>
            {list.upcoming.length === 0 ? (
              <p className="text-sm">これからの枠はありません。</p>
            ) : (
              <SlotTable
                label="これからの枠"
                slots={list.upcoming}
                withdraw={{ now: list.fetchedAt, onWithdraw: withdrawSlot }}
              />
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
