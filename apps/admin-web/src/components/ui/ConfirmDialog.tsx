import { useEffect, useId, useRef } from "react";

import { Button } from "./Button";

/**
 * 確認のダイアログ（specs/design/components.yaml の confirm_dialog）。題は疑問の文、本文に対象と結果と条件を省かずに書く。
 * 開いたらフォーカスを題へ移す。cancel（「やめる」）を左、confirm を右。外を押す・Escape は cancel と同じ。
 * 危険な操作（取り消せない）の確認は danger、それ以外は primary。
 */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  cancelLabel = "やめる",
  danger = false,
  busy = false,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const titleId = useId();
  const bodyId = useId();
  const titleRef = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    if (open) titleRef.current?.focus();
  }, [open]);

  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-scrim p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") onCancel();
      }}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={bodyId}
        className="w-full max-w-[400px] space-y-4 rounded-md bg-background p-6"
      >
        <h2 id={titleId} ref={titleRef} tabIndex={-1} className="text-xl font-bold">
          {title}
        </h2>
        <p id={bodyId}>{body}</p>
        <div className="flex justify-end gap-2">
          <Button variant="text" disabled={busy} onClick={onCancel}>
            {cancelLabel}
          </Button>
          <Button variant={danger ? "danger" : "primary"} loading={busy} onClick={onConfirm}>
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
}
