import type { ButtonHTMLAttributes } from "react";

import { Spinner } from "./Loading";

// ボタン（specs/design/components.yaml の button）。文言は動詞で終える。

export type ButtonVariant = "primary" | "secondary" | "danger" | "text";

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: "bg-primary text-on-primary active:bg-primary-pressed",
  secondary: "border border-border-strong bg-background text-primary active:bg-primary-container",
  danger: "bg-danger text-on-primary active:bg-danger-on-container",
  text: "text-primary active:bg-primary-container",
};

const BASE =
  "inline-flex min-h-12 items-center justify-center gap-2 rounded-pill px-6 text-base font-bold transition-colors disabled:border-transparent disabled:bg-surface disabled:text-text-muted";

/**
 * 主なボタン（primary）は 1 画面に 1 つまで。危険な操作の確認だけ danger。
 * loading のあいだはラベルを残したまま spinner を前に出し、押せなくする（二度押しで同じ操作を 2 回送らない）。
 * admin-web は濃い面を使わないので on_dark はない。
 */
export function Button({
  variant = "primary",
  loading = false,
  disabled,
  className,
  children,
  type = "button",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant; loading?: boolean }) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={[BASE, VARIANT_CLASSES[variant], className].filter(Boolean).join(" ")}
      {...props}
    >
      {loading && <Spinner />}
      {children}
    </button>
  );
}
