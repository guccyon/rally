import type { ReactNode } from "react";

/**
 * カード（specs/design/components.yaml の card）。1 件のまとまりを surface の面で囲む（影なし）。
 * onClick があるときはカード全体を 1 つのボタンにし、中にボタンを置かない。
 */
export function Card({ children, onClick }: { children: ReactNode; onClick?: () => void }) {
  const className = "flex w-full flex-col gap-2 rounded-md bg-surface p-4 text-left";
  if (onClick) {
    return (
      <button type="button" onClick={onClick} className={`${className} active:bg-primary-container`}>
        {children}
      </button>
    );
  }
  return <div className={className}>{children}</div>;
}
