import type { ReactNode } from "react";

/**
 * カード（specs/design/components.yaml の card の plain）。1 件のまとまりを background に border の 1px で囲む
 * （影なし。0.5.0 で surface の面をやめた）。カードの中にカードを置かない。
 * onClick があるときはカード全体を 1 つのボタンにし、中にボタンを置かない。
 */
export function Card({ children, onClick }: { children: ReactNode; onClick?: () => void }) {
  const className = "flex w-full flex-col gap-2 rounded-md border border-border bg-background p-4 text-left";
  if (onClick) {
    return (
      <button type="button" onClick={onClick} className={`${className} active:bg-primary-container`}>
        {children}
      </button>
    );
  }
  return <div className={className}>{children}</div>;
}
