"use client";

import { useState } from "react";

import type { Schemas } from "@/api/client";
import { lengthOf } from "@/components/coach/lessonMenuInput";
import { Button, type ButtonVariant } from "@/components/ui/Button";

/** 本部が受付を止めるときの理由の上限（契約の SuspendCoachRequest.reason。コードポイントで数える）。 */
export const SUSPEND_REASON_MAX_LENGTH = 200;

const ACTIONS: Record<
  Schemas["CoachAcceptanceStatus"],
  { button: string; question: string; confirm: string; confirmVariant: ButtonVariant }
> = {
  accepting: {
    button: "受付を止める",
    question:
      "このコーチの受付を止めますか？新しい予約を受け付けなくなり、枠は生徒の検索に出なくなります。入っている予約はそのままです。",
    confirm: "停止を確定",
    confirmVariant: "danger",
  },
  suspended: {
    button: "再開する",
    question: "このコーチの受付を再開しますか？公開中の枠がまた生徒の検索に出て、予約できるようになります。",
    confirm: "再開を確定",
    confirmVariant: "primary",
  },
};

/**
 * コーチの行の「受付を止める」/「再開する」。押したら行の中で確かめ（ブラウザの confirm は使わない）、確定で送る。
 * どちらを出すかは今の受付の状態で決める。止めるときだけ理由（任意）を書け、`onConfirm` に渡す（再開は常に空文字列）。
 * 送る処理と結果の表示は呼び出し側（`onConfirm`）が持つ。
 */
export function CoachAcceptanceAction({
  status,
  onConfirm,
}: {
  status: Schemas["CoachAcceptanceStatus"];
  onConfirm: (reason: string) => Promise<void>;
}) {
  const [confirming, setConfirming] = useState(false);
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const action = ACTIONS[status];
  const suspending = status === "accepting";
  const remaining = SUSPEND_REASON_MAX_LENGTH - lengthOf(reason);

  async function confirm() {
    setBusy(true);
    await onConfirm(suspending ? reason : "");
    setBusy(false);
    setConfirming(false);
    setReason("");
  }

  function cancel() {
    setConfirming(false);
    setReason("");
  }

  if (!confirming) {
    return (
      <Button variant="secondary" onClick={() => setConfirming(true)}>
        {action.button}
      </Button>
    );
  }
  return (
    <div role="group" aria-label="コーチの受付の確認" className="space-y-1">
      <p className="text-xs">{action.question}</p>
      {suspending && (
        <>
          <label className="block text-xs">
            理由（任意）
            <textarea
              className="mt-1 block w-full rounded-md border px-2 py-1"
              rows={2}
              value={reason}
              disabled={busy}
              aria-invalid={remaining < 0 ? true : undefined}
              onChange={(event) => setReason(event.target.value)}
            />
          </label>
          <span className={remaining < 0 ? "text-xs text-danger" : "text-xs text-text-muted"}>
            {remaining < 0
              ? `${SUSPEND_REASON_MAX_LENGTH} 文字を ${-remaining} 文字超えています`
              : `あと ${remaining} 文字`}
          </span>
        </>
      )}
      <div className="flex gap-2">
        <Button
          variant={action.confirmVariant}
          disabled={busy || (suspending && remaining < 0)}
          onClick={() => void confirm()}
        >
          {action.confirm}
        </Button>
        <Button variant="secondary" disabled={busy} onClick={cancel}>
          やめる
        </Button>
      </div>
    </div>
  );
}
