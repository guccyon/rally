"use client";

import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; changes: Schemas["CoachAcceptanceChange"][] }
  | { kind: "failed"; message: string };

/** 記録の `acceptance_status` は変えた後の状態なので、止めた・再開したの言い方に直す。 */
const CHANGE_LABELS: Record<Schemas["CoachAcceptanceStatus"], string> = {
  suspended: "止めた",
  accepting: "再開した",
};

/**
 * 本部の「コーチの様子」の「受付の記録」。開いたときにコーチ 1 人の受付を止めた・再開した記録
 * （listCoachAcceptanceChanges）を一度だけ読み、返った順（契約どおり新しい順）のまま、止めた・再開した・日時・
 * 操作した本部のアカウント（契約にあるのは ID だけ）を出す。0 件なら「受付を止めたことはありません」、
 * 失敗したときはその文言を出す。受付は変えない。
 */
export function CoachAcceptanceHistory({
  api,
  coachId,
}: {
  api: Pick<HeadquartersApi, "listCoachAcceptanceChanges">;
  coachId: string;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let active = true;
    void api.listCoachAcceptanceChanges(coachId).then((response) => {
      if (!active) return;
      setState(
        response.ok
          ? { kind: "loaded", changes: response.data.changes }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api, coachId]);

  if (state.kind === "loading") return <p className="text-sm text-gray-500">読み込み中…</p>;
  if (state.kind === "failed") {
    return (
      <ResultMessage result={{ ok: false, message: `受付の記録を読めませんでした。${state.message}` }} />
    );
  }
  if (state.changes.length === 0) return <p className="text-sm">受付を止めたことはありません</p>;

  return (
    <table aria-label="受付の記録" className="w-full text-left text-sm">
      <thead>
        <tr>
          <th className="py-1">操作</th>
          <th className="py-1">日時</th>
          <th className="py-1">操作した本部のアカウント</th>
        </tr>
      </thead>
      <tbody>
        {state.changes.map((change, index) => (
          // 記録に ID はなく、同じ時刻もありうるので、返った順の位置で区別する（並べ替えはしない）。
          <tr key={index} className="border-t">
            <td className="py-1">{CHANGE_LABELS[change.acceptance_status]}</td>
            <td className="py-1">{formatDateTime(change.changed_at)}</td>
            <td className="py-1 font-mono text-xs">{change.changed_by}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
