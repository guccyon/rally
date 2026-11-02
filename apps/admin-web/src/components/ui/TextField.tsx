import { useId, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from "react";

import { Icon } from "./Icon";

// 入力欄（specs/design/components.yaml の text_field）。見えるラベルを必ず付ける（placeholder で代えない）。

type FieldProps = {
  label: string;
  /** 上限など（「200 文字まで」） */
  helper?: string;
  /** 欄を離れたときと送るときに確かめた誤り。何をどう直すかを書く */
  error?: string;
  /** 上限のある欄の文字数（「12 / 200」） */
  counter?: string;
};

const FIELD =
  "block w-full rounded-md border border-border-strong bg-background px-4 text-base text-text focus:border-focus-ring focus:outline-2 focus:outline-focus-ring disabled:bg-surface disabled:text-text-muted aria-invalid:border-2 aria-invalid:border-danger";

function FieldFrame({
  id,
  label,
  helper,
  error,
  counter,
  children,
}: FieldProps & { id: string; children: ReactNode }) {
  return (
    <div className="space-y-1">
      <label htmlFor={id} className="block text-sm font-bold text-text">
        {label}
      </label>
      {children}
      {(helper || counter) && (
        <div className="flex justify-between gap-2 text-sm text-text-muted">
          <span id={`${id}-helper`}>{helper}</span>
          {counter && <span>{counter}</span>}
        </div>
      )}
      {error && (
        <p id={`${id}-error`} className="flex items-start gap-1 text-sm text-danger">
          <Icon name="error" size={20} />
          {error}
        </p>
      )}
    </div>
  );
}

function describedBy(id: string, { helper, error }: FieldProps) {
  return [helper && `${id}-helper`, error && `${id}-error`].filter(Boolean).join(" ") || undefined;
}

/** 1 行の入力欄（高さ 48px）。 */
export function TextField({
  label,
  helper,
  error,
  counter,
  className,
  ...props
}: FieldProps & InputHTMLAttributes<HTMLInputElement>) {
  const id = useId();
  return (
    <FieldFrame id={id} label={label} helper={helper} error={error} counter={counter}>
      <input
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy(id, { label, helper, error })}
        className={[FIELD, "min-h-12", className].filter(Boolean).join(" ")}
        {...props}
      />
    </FieldFrame>
  );
}

/** 数行の入力欄（3 行から伸びる）。 */
export function TextArea({
  label,
  helper,
  error,
  counter,
  className,
  rows = 3,
  ...props
}: FieldProps & TextareaHTMLAttributes<HTMLTextAreaElement>) {
  const id = useId();
  return (
    <FieldFrame id={id} label={label} helper={helper} error={error} counter={counter}>
      <textarea
        id={id}
        rows={rows}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy(id, { label, helper, error })}
        className={[FIELD, "py-2", className].filter(Boolean).join(" ")}
        {...props}
      />
    </FieldFrame>
  );
}
