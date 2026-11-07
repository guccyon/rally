"use client";

import { useState } from "react";
import { Button } from "@/components/ui/Button";

/**
 * 低い評価の行の「対応済みにする」。押したら行の中で確かめ（ブラウザの confirm は使わない）、確定で送る。
 * 印を外す操作は契約にないので、確かめの文でそれを伝える。送る処理と結果の表示は呼び出し側（`onConfirm`）が持つ。
 */
export function LowRatedReviewHandlingAction({
  label,
  onConfirm,
}: {
  /** 行を見分ける名前（ボタンと確かめの欄の読み上げに使う） */
  label: string;
  onConfirm: () => Promise<void>;
}) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  async function confirm() {
    setBusy(true);
    await onConfirm();
    setBusy(false);
    setConfirming(false);
  }

  if (!confirming) {
    return (
      <Button variant="secondary" aria-label={`${label}を対応済みにする`} onClick={() => setConfirming(true)}>
        対応済みにする
      </Button>
    );
  }
  return (
    <div role="group" aria-label={`${label}の対応済みの確認`} className="space-y-1">
      <p className="text-xs">
        この評価を対応済みにしますか？「未対応だけ」の一覧に出なくなります。対応済みは外せません。
      </p>
      <div className="flex gap-2">
        <Button disabled={busy} onClick={() => void confirm()}>
          {busy ? "対応済みにしています…" : "対応済みを確定"}
        </Button>
        <Button variant="secondary" disabled={busy} onClick={() => setConfirming(false)}>
          やめる
        </Button>
      </div>
    </div>
  );
}
