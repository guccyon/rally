"use client";

import { useState } from "react";

import { Button } from "@/components/ui/Button";

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
      <Button variant="secondary" onClick={() => setConfirming(true)}>
        取り下げる
      </Button>
    );
  }
  return (
    <div role="group" aria-label="枠の取り下げの確認" className="space-y-1">
      <p className="text-xs">
        この枠を取り下げますか？生徒の検索に出なくなり、公開中には戻せません。
      </p>
      <div className="flex gap-2">
        <Button variant="danger" disabled={busy} onClick={() => void confirm()}>
          取り下げを確定
        </Button>
        <Button variant="secondary" disabled={busy} onClick={() => setConfirming(false)}>
          やめる
        </Button>
      </div>
    </div>
  );
}
