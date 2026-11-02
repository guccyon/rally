import type { ReactNode } from "react";

/** 空の表示（specs/design/components.yaml の empty_state）。背中を押す一言と次の操作（secondary か text のボタン。任意）。 */
export function EmptyState({ message, action }: { message: string; action?: ReactNode }) {
  return (
    <div className="space-y-4 rounded-md bg-surface p-6 text-text-muted">
      <p>{message}</p>
      {action}
    </div>
  );
}
