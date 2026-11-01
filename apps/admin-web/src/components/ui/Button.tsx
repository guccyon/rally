import type { ButtonHTMLAttributes } from "react";

/** components.yaml の button の種類。 */
export type ButtonVariant = "primary" | "secondary" | "danger";

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: "bg-blue-600 text-white",
  secondary: "border",
  danger: "bg-red-600 text-white",
};

const SIZE_CLASSES = {
  /** 段の主な操作 */
  md: "px-4 py-1",
  /** 一覧の行の中の操作 */
  sm: "px-2 py-0.5",
};

type Props = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className"> & {
  variant?: ButtonVariant;
  size?: keyof typeof SIZE_CLASSES;
  /** API の応答を待つ間。押せなくし、`busyLabel` があれば文言をそれにする */
  busy?: boolean;
  busyLabel?: string;
};

/** ボタン（components.yaml の button）。主・副・危険と、通常・無効・処理中。 */
export function Button({
  variant = "primary",
  size = "md",
  busy = false,
  busyLabel,
  disabled,
  type = "button",
  children,
  ...rest
}: Props) {
  return (
    <button
      {...rest}
      type={type}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      className={`rounded ${VARIANT_CLASSES[variant]} ${SIZE_CLASSES[size]} disabled:opacity-50`}
    >
      {busy && busyLabel ? busyLabel : children}
    </button>
  );
}
