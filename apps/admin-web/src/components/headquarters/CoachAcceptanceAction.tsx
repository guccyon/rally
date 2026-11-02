"use client";

import { useState } from "react";

import type { Schemas } from "@/api/client";

const ACTIONS: Record<
  Schemas["CoachAcceptanceStatus"],
  { button: string; question: string; confirm: string; buttonClass: string; confirmClass: string }
> = {
  accepting: {
    button: "受付を止める",
    question:
      "このコーチの受付を止めますか？新しい予約を受け付けなくなり、枠は生徒の検索に出なくなります。入っている予約はそのままです。",
    confirm: "停止を確定",
    buttonClass: "border-danger text-danger",
    confirmClass: "bg-danger",
  },
  suspended: {
    button: "再開する",
    question: "このコーチの受付を再開しますか？公開中の枠がまた生徒の検索に出て、予約できるようになります。",
    confirm: "再開を確定",
    buttonClass: "border-primary text-primary",
    confirmClass: "bg-primary",
  },
};

/**
 * コーチの行の「受付を止める」/「再開する」。押したら行の中で確かめ（ブラウザの confirm は使わない）、確定で送る。
 * どちらを出すかは今の受付の状態で決める。送る処理と結果の表示は呼び出し側（`onConfirm`）が持つ。
 */
export function CoachAcceptanceAction({
  status,
  onConfirm,
}: {
  status: Schemas["CoachAcceptanceStatus"];
  onConfirm: () => Promise<void>;
}) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const action = ACTIONS[status];

  async function confirm() {
    setBusy(true);
    await onConfirm();
    setBusy(false);
    setConfirming(false);
  }

  if (!confirming) {
    return (
      <button
        type="button"
        className={`rounded-pill border px-2 py-0.5 ${action.buttonClass}`}
        onClick={() => setConfirming(true)}
      >
        {action.button}
      </button>
    );
  }
  return (
    <div role="group" aria-label="コーチの受付の確認" className="space-y-1">
      <p className="text-xs">{action.question}</p>
      <div className="flex gap-2">
        <button
          type="button"
          disabled={busy}
          className={`rounded-pill px-2 py-0.5 text-on-primary disabled:opacity-50 ${action.confirmClass}`}
          onClick={() => void confirm()}
        >
          {action.confirm}
        </button>
        <button
          type="button"
          disabled={busy}
          className="rounded-pill border px-2 py-0.5 disabled:opacity-50"
          onClick={() => setConfirming(false)}
        >
          やめる
        </button>
      </div>
    </div>
  );
}
