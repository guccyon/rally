"use client";

import { useState } from "react";

/**
 * 枠の行の「取り下げる」。押したら行の中で確かめ（ブラウザの confirm は使わない）、「取り下げを確定」で送る。
 * 送る処理と結果の表示は呼び出し側（`onWithdraw`）が持つ。
 */
export function WithdrawSlotAction({ onWithdraw }: { onWithdraw: () => Promise<void> }) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  async function confirm() {
    setBusy(true);
    await onWithdraw();
    setBusy(false);
    setConfirming(false);
  }

  if (!confirming) {
    return (
      <button
        type="button"
        className="rounded-pill border border-danger px-2 py-0.5 text-danger"
        onClick={() => setConfirming(true)}
      >
        取り下げる
      </button>
    );
  }
  return (
    <div role="group" aria-label="枠の取り下げの確認" className="space-y-1">
      <p className="text-xs">
        この枠を取り下げますか？生徒の検索に出なくなり、公開中には戻せません。
      </p>
      <div className="flex gap-2">
        <button
          type="button"
          disabled={busy}
          className="rounded-pill bg-danger px-2 py-0.5 text-on-primary disabled:opacity-50"
          onClick={() => void confirm()}
        >
          取り下げを確定
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
