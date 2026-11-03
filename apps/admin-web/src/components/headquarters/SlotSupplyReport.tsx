"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import {
  formatDateTime,
  formatDay,
  SLOT_SUPPLY_STATUS_LABELS,
  SPORT_LABELS,
  toJstDate,
} from "@/components/labels";

import { MAX_PERIOD_DAYS, type Period, periodError } from "./bookingPeriod";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; report: Schemas["SlotSupplyReport"] }
  | { kind: "failed"; message: string };

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間の指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。`;

/**
 * スポーツごとの予約枠の需給（getSlotSupply）。判定と数え方は契約の説明が正本で、画面は返った値を出すだけ。
 * 開いたときは期間を付けずに読み（API の既定: 集計した時刻までの 30 日）、数えた期間の日付（日本時間）を入力欄に入れる。
 * 「この条件で見る」で選んだ期間（日本時間の日付）を `from` / `to` に渡して読み直し、「再集計」は最後に読んだ条件で読み直す。
 */
export function SlotSupplyReport({ api }: { api: Pick<HeadquartersApi, "getSlotSupply"> }) {
  // period が null なら API の既定の期間に任せる（開いたときだけ）。
  const [draft, setDraft] = useState<Period | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);
  const [applied, setApplied] = useState<Period | null>(null);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 条件を続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchReport = useCallback(
    async (period: Period | null): Promise<State> => {
      const response = await api.getSlotSupply(period ?? {});
      if (response.ok) return { kind: "loaded", report: response.data };
      return {
        kind: "failed",
        message: response.code === "validation_failed" ? VALIDATION_FAILED : response.message,
      };
    },
    [api],
  );

  // 最初の集計。初期の状態が loading なので、ここでは結果だけを入れる。読めたら数えた期間の日付を入力欄に入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchReport(null).then((next) => {
      if (!active || request !== latestRequest.current) return;
      setState(next);
      if (next.kind === "loaded") {
        const used = usedPeriodOf(next.report);
        setDraft((current) => current ?? used);
      }
    });
    return () => {
      active = false;
    };
  }, [fetchReport]);

  async function load(period: Period | null) {
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchReport(period);
    if (request === latestRequest.current) setState(result);
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    const period = draft ?? { from: "", to: "" };
    const error = periodError(period);
    setDraftError(error);
    if (error !== null) return;
    setApplied(period);
    await load(period);
  }

  const period = draft ?? { from: "", to: "" };

  return (
    <div className="space-y-4">
      <p className="text-sm">
        枠の開始の日付（日本時間）で期間を選んで、繁忙期の前に枠が足りるかを見られます。期間は {MAX_PERIOD_DAYS}{" "}
        日まで選べます。開いたときは集計した時刻までの 30 日です。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submit(event)}>
        <label className="block">
          開始日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={period.from}
            onChange={(event) => setDraft({ ...period, from: event.target.value })}
          />
        </label>
        <label className="block">
          終了日
          <input
            type="date"
            className="mt-1 block rounded-md border px-2 py-1"
            value={period.to}
            onChange={(event) => setDraft({ ...period, to: event.target.value })}
          />
        </label>
        <button type="submit" className="rounded-pill bg-primary px-4 py-1 text-on-primary">
          この条件で見る
        </button>
        <button
          type="button"
          disabled={state.kind === "loading"}
          onClick={() => void load(applied)}
          className="rounded-pill border px-3 py-1 text-sm disabled:opacity-50"
        >
          再集計
        </button>
      </form>
      {draftError !== null && <ResultMessage result={{ ok: false, message: draftError }} />}
      {state.kind === "loading" && <p className="text-sm text-text-muted">集計中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && <LoadedReport report={state.report} />}
    </div>
  );
}

/** 数えた期間の日付（日本時間）。日付で指定していれば応答の `from` / `to`、既定の 30 日なら `window_start` / `window_end` の日付。 */
function usedPeriodOf(report: Schemas["SlotSupplyReport"]): Period {
  if (report.from !== undefined && report.to !== undefined) return { from: report.from, to: report.to };
  // window_end は含まないので、その直前の時刻の日付を終わりにする。
  return { from: toJstDate(report.window_start), to: toJstDate(Date.parse(report.window_end) - 1) };
}

/** どのスポーツにも期間に開始する枠がなく、予約しようとした生徒もいない（数えるものが 0 件）。 */
function isEmpty(report: Schemas["SlotSupplyReport"]): boolean {
  return report.sports.every((supply) => supply.published_slots === 0 && supply.booking_students === 0);
}

function LoadedReport({ report }: { report: Schemas["SlotSupplyReport"] }) {
  return (
    <div className="space-y-2">
      {report.from !== undefined && report.to !== undefined ? (
        <p className="text-sm">
          集計の期間: {formatDay(report.from)} 〜 {formatDay(report.to)}（この期間の日付に開始する枠を数える）
        </p>
      ) : (
        <p className="text-sm">
          集計の期間: {formatDateTime(report.window_start)} 〜 {formatDateTime(report.window_end)}
          （終わりは含まない。この期間に開始する枠を数える）
        </p>
      )}
      {isEmpty(report) && <p className="text-sm">この期間に開始する予約枠はなく、予約しようとした生徒もいません</p>}
      {!isEmpty(report) && (
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left">
              <th>スポーツ</th>
              <th>判定</th>
              <th>公開した枠の数</th>
              <th>予約しようとした生徒の数</th>
            </tr>
          </thead>
          <tbody>
            {report.sports.map((supply) => (
              <tr key={supply.sport}>
                <td>{SPORT_LABELS[supply.sport]}</td>
                <td className={supply.status === "insufficient" ? "font-semibold text-danger" : ""}>
                  {SLOT_SUPPLY_STATUS_LABELS[supply.status]}
                </td>
                <td>{supply.published_slots}</td>
                <td>{supply.booking_students}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
