"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime, formatDay } from "@/components/labels";

import { MAX_PERIOD_DAYS, type Period, periodError } from "./bookingPeriod";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; changes: Schemas["CoachAcceptanceChange"][]; period: Period | null }
  | { kind: "failed"; message: string };

/** 記録の `acceptance_status` は変えた後の状態なので、止めた・再開したの言い方に直す。 */
const CHANGE_LABELS: Record<Schemas["CoachAcceptanceStatus"], string> = {
  suspended: "止めた",
  accepting: "再開した",
};

// 期間で絞っているとき（送る前の確かめ periodError をすり抜けた 422）の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間の指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。`;

const NO_PERIOD: Period = { from: "", to: "" };

/**
 * 本部の「コーチの様子」の「受付の記録」。開いたときにコーチ 1 人の受付を止めた・再開した記録
 * （listCoachAcceptanceChanges）を期間を付けずに（これまでのすべて）読み、返った順（契約どおり新しい順）のまま、
 * 止めた・再開した・日時・操作した本部のアカウント（契約にあるのは ID だけ）・止めたときに書いた理由を出す。理由がない記録
 * （再開した記録、理由を書かずに止めた記録、理由を残し始める前の記録）は「—」。
 * 「開始日」「終了日」で変えた日付（日本時間）の期間に絞る（API の `from`・`to`）。期間の欄は本部のコーチの評価
 * （CoachReviewPicker）と同じく、送る前に確かめて「この期間で見る」で読み直し、「期間を外す」ですべてに戻す。
 * 取り直すときは確定した期間（最後に「この期間で見る」を押した期間。外したあとは期間なし）で読む。
 * 0 件なら「受付を止めたことはありません」（期間で絞っていればその期間を含めた文言）、失敗したときはその文言を出す。受付は変えない。
 */
export function CoachAcceptanceHistory({
  api,
  coachId,
}: {
  api: Pick<HeadquartersApi, "listCoachAcceptanceChanges">;
  coachId: string;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });
  // 確定した期間（null なら期間で絞らない）。期間の欄の値は「この期間で見る」を押すまで記録に効かない。
  const [period, setPeriod] = useState<Period | null>(null);
  const [draftPeriod, setDraftPeriod] = useState<Period>(NO_PERIOD);
  const [periodDraftError, setPeriodDraftError] = useState<string | null>(null);
  // 読み直したときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchChanges = useCallback(
    async (next: Period | null): Promise<State> => {
      const response = await api.listCoachAcceptanceChanges(coachId, next ?? {});
      if (response.ok) return { kind: "loaded", changes: response.data.changes, period: next };
      if (response.code === "validation_failed" && next !== null) return { kind: "failed", message: VALIDATION_FAILED };
      return { kind: "failed", message: `受付の記録を読めませんでした。${response.message}` };
    },
    [api, coachId],
  );

  /** 確定した期間で読み直す（期間を確定したとき・外したとき）。 */
  async function reload(next: Period | null) {
    setPeriod(next);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchChanges(next);
    if (request === latestRequest.current) setState(result);
  }

  // 開いたときは期間を付けずに読む。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    const request = ++latestRequest.current;
    void fetchChanges(null).then((result) => {
      if (request === latestRequest.current) setState(result);
    });
    return () => {
      // 閉じたあとに返った結果は捨てる。
      latestRequest.current += 1;
    };
  }, [fetchChanges]);

  function submitPeriod(event: FormEvent) {
    event.preventDefault();
    const error = periodError(draftPeriod);
    setPeriodDraftError(error);
    if (error !== null) return;
    void reload(draftPeriod);
  }

  function clearPeriod() {
    setDraftPeriod(NO_PERIOD);
    setPeriodDraftError(null);
    void reload(null);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        期間を選ぶと、その期間（日本時間）に止めた・再開した記録だけにします（期間は {MAX_PERIOD_DAYS} 日まで）。
      </p>
      <form className="flex flex-wrap items-end gap-4" onSubmit={submitPeriod}>
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
        <button type="submit" className="rounded-pill bg-primary px-4 py-1 text-on-primary">
          この期間で見る
        </button>
        {period !== null && (
          <button type="button" className="rounded-pill border px-4 py-1" onClick={clearPeriod}>
            期間を外す
          </button>
        )}
      </form>
      {periodDraftError !== null && <ResultMessage result={{ ok: false, message: periodDraftError }} />}
      <ChangeTable state={state} />
    </div>
  );
}

/** 記録の表（読み込み中・失敗・0 件を含む）。 */
function ChangeTable({ state }: { state: State }) {
  if (state.kind === "loading") return <p className="text-sm text-text-muted">読み込み中…</p>;
  if (state.kind === "failed") return <ResultMessage result={{ ok: false, message: state.message }} />;
  if (state.changes.length === 0) return <p className="text-sm">{emptyMessageOf(state.period)}</p>;

  return (
    <table aria-label="受付の記録" className="w-full text-left text-sm">
      <thead>
        <tr>
          <th className="py-1">操作</th>
          <th className="py-1">日時</th>
          <th className="py-1">操作した本部のアカウント</th>
          <th className="py-1">理由</th>
        </tr>
      </thead>
      <tbody>
        {state.changes.map((change, index) => (
          // 記録に ID はなく、同じ時刻もありうるので、返った順の位置で区別する（並べ替えはしない）。
          <tr key={index} className="border-t">
            <td className="py-1">{CHANGE_LABELS[change.acceptance_status]}</td>
            <td className="py-1">{formatDateTime(change.changed_at)}</td>
            <td className="py-1 font-mono text-xs">{change.changed_by}</td>
            <td className="py-1">{change.reason || "—"}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** 0 件のときの文言。期間で絞っていれば、その期間を含める。 */
function emptyMessageOf(period: Period | null): string {
  if (period === null) return "受付を止めたことはありません";
  return `${formatDay(period.from)} 〜 ${formatDay(period.to)}に受付を止めた・再開した記録はありません`;
}
