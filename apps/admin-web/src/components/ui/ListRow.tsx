import type { ReactNode } from "react";

/** 一覧の行（ListRow）を並べる枠。label は一覧の名前（支援技術が読む）。 */
export function List({ label, children }: { label: string; children: ReactNode }) {
  return (
    <ul aria-label={label} className="divide-y rounded border text-sm">
      {children}
    </ul>
  );
}

type Props = {
  children: ReactNode;
  /** 行の右端（「〜へ」・状態のしるし・行の操作） */
  trailing?: ReactNode;
  /** あれば押せる行（components.yaml の list_row / action）。行の全体が押せる */
  onOpen?: () => void;
  disabled?: boolean;
};

/** 一覧の行（components.yaml の list_row）。表示だけの行と押せる行。 */
export function ListRow({ children, trailing, onOpen, disabled = false }: Props) {
  if (onOpen) {
    return (
      <li>
        <button
          type="button"
          onClick={onOpen}
          disabled={disabled}
          className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left hover:bg-gray-50 disabled:opacity-50"
        >
          <span>{children}</span>
          {trailing && <span className="shrink-0 text-blue-700">{trailing}</span>}
        </button>
      </li>
    );
  }
  return (
    <li className={`flex items-center justify-between gap-2 px-3 py-2${disabled ? " opacity-50" : ""}`}>
      <div className="space-y-1">{children}</div>
      {trailing && <div className="shrink-0">{trailing}</div>}
    </li>
  );
}
