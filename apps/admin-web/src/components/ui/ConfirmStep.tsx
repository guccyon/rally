import type { ReactNode } from "react";

import { Button } from "./Button";

type Props = {
  /** 段の名前（role="group" の aria-label。例: 「送る前の確認」） */
  label: string;
  /** 何が起きるか（誰に届くか・取り消せないこと）の 1 文 */
  message: ReactNode;
  /** 内容の要約（見出しと値の組） */
  items?: { term: string; detail: ReactNode }[];
  /** 要約のあとに置くもの（理由の入力など） */
  children?: ReactNode;
  confirmLabel: string;
  busyLabel?: string;
  cancelLabel?: string;
  /** 確定のボタンの種類（components.yaml の confirm_step の種類） */
  variant?: "primary" | "danger";
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
};

/** 確認の段（components.yaml の confirm_step）。同じ画面の中で内容を見せ、確定か直すを選ぶ。送っている間は両方押せない。 */
export function ConfirmStep({
  label,
  message,
  items,
  children,
  confirmLabel,
  busyLabel,
  cancelLabel = "直す",
  variant = "primary",
  busy = false,
  onConfirm,
  onCancel,
}: Props) {
  return (
    <div role="group" aria-label={label} className="space-y-3">
      <p className="text-sm">{message}</p>
      {items && items.length > 0 && (
        <dl className="space-y-2 rounded border bg-gray-50 p-3 text-sm">
          {items.map((item) => (
            <div key={item.term}>
              <dt className="text-gray-600">{item.term}</dt>
              <dd className="whitespace-pre-wrap">{item.detail}</dd>
            </div>
          ))}
        </dl>
      )}
      {children}
      <div className="flex gap-2">
        <Button variant={variant} busy={busy} busyLabel={busyLabel} onClick={onConfirm}>
          {confirmLabel}
        </Button>
        <Button variant="secondary" disabled={busy} onClick={onCancel}>
          {cancelLabel}
        </Button>
      </div>
    </div>
  );
}
