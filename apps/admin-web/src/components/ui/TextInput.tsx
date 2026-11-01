import type { InputHTMLAttributes } from "react";

import { Field, type FieldProps } from "./Field";

type TextInputProps = FieldProps & Omit<InputHTMLAttributes<HTMLInputElement>, "className">;

/** 1 行の入力（components.yaml の input / single_line）。 */
export function TextInput({ label, error, hint, type = "text", ...rest }: TextInputProps) {
  return (
    <Field label={label} error={error} hint={hint}>
      {(a11y) => <input {...rest} {...a11y} type={type} className="mt-1 block w-full rounded border px-2 py-1" />}
    </Field>
  );
}
