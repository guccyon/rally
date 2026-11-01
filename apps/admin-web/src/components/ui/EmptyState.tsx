import type { ReactNode } from "react";

/** 空の表示（components.yaml の empty_state）。何がないかと、どうすれば出るかを書く。 */
export function EmptyState({ children }: { children: ReactNode }) {
  return <p className="text-sm">{children}</p>;
}
