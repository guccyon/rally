"use client";

import { useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime, SLOT_SUPPLY_STATUS_LABELS, SPORT_LABELS } from "@/components/labels";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; report: Schemas["SlotSupplyReport"] }
  | { kind: "failed"; message: string };

/** スポーツごとの予約枠の需給（getSlotSupply）。判定と数え方は契約の説明が正本で、画面は返った値を出すだけ。 */
export function SlotSupplyReport({ api }: { api: Pick<HeadquartersApi, "getSlotSupply"> }) {
  const [state, setState] = useState<State>({ kind: "loading" });

  const fetchReport = useCallback(async (): Promise<State> => {
    const response = await api.getSlotSupply();
    return response.ok
      ? { kind: "loaded", report: response.data }
      : { kind: "failed", message: response.message };
  }, [api]);

  // 最初の集計。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchReport().then((next) => {
      if (active) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchReport]);

  async function reload() {
    setState({ kind: "loading" });
    setState(await fetchReport());
  }

  return (
    <div className="space-y-4">
      <button
        type="button"
        disabled={state.kind === "loading"}
        onClick={() => void reload()}
        className="rounded border px-3 py-1 text-sm disabled:opacity-50"
      >
        再集計
      </button>
      {state.kind === "loading" && <p className="text-sm text-gray-500">集計中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && (
        <>
          <p className="text-sm">
            集計の期間: {formatDateTime(state.report.window_start)} 〜{" "}
            {formatDateTime(state.report.window_end)}（終わりは含まない。この期間に開始する枠を数える）
          </p>
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
              {state.report.sports.map((supply) => (
                <tr key={supply.sport}>
                  <td>{SPORT_LABELS[supply.sport]}</td>
                  <td className={supply.status === "insufficient" ? "font-semibold text-red-600" : ""}>
                    {SLOT_SUPPLY_STATUS_LABELS[supply.status]}
                  </td>
                  <td>{supply.published_slots}</td>
                  <td>{supply.booking_students}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}
