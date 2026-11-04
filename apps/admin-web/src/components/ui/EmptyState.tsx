import { Button } from "./Button";

/** 空の表示の種類（specs/design/components.yaml の empty_state.variants）。 */
export type EmptyStateVariant = "screen" | "section";

const VARIANT_CLASSES: Record<EmptyStateVariant, string> = {
  screen: "bg-surface",
  section: "border border-border bg-background",
};

/**
 * 空の表示（specs/design/components.yaml の empty_state）。背中を押す一言と次の操作（任意）。
 * screen は画面にほかの主な操作がないとき（surface の面と primary のボタン）、
 * section は欄だけが空で画面に primary があるとき（background の面と border の線と secondary のボタン）。
 * surface の面に secondary のボタンを置かないよう、ボタンの種類は variant で決める。
 */
export function EmptyState({
  message,
  action,
  variant = "screen",
}: {
  message: string;
  action?: { label: string; onClick: () => void };
  variant?: EmptyStateVariant;
}) {
  return (
    <div className={`flex flex-col items-start gap-4 rounded-md p-6 text-text ${VARIANT_CLASSES[variant]}`}>
      <p>{message}</p>
      {action && (
        <Button variant={variant === "screen" ? "primary" : "secondary"} onClick={action.onClick}>
          {action.label}
        </Button>
      )}
    </div>
  );
}
