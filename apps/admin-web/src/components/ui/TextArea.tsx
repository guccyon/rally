import type { TextareaHTMLAttributes } from "react";

import { Field, type FieldProps } from "./Field";

type TextAreaProps = FieldProps & Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className">;

/** 複数行の入力（components.yaml の input / multi_line）。 */
export function TextArea({ label, error, hint, ...rest }: TextAreaProps) {
  return (
    <Field label={label} error={error} hint={hint}>
      {(a11y) => <textarea {...rest} {...a11y} className="mt-1 block w-full rounded border px-2 py-1" />}
    </Field>
  );
}
