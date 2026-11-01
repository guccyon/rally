import type { SelectHTMLAttributes } from "react";

import { Field, type FieldProps } from "./Field";

type SelectProps = FieldProps & Omit<SelectHTMLAttributes<HTMLSelectElement>, "className">;

/** 選択（components.yaml の input / select）。選択肢は children の option で渡す。 */
export function Select({ label, error, hint, children, ...rest }: SelectProps) {
  return (
    <Field label={label} error={error} hint={hint}>
      {(a11y) => (
        <select {...rest} {...a11y} className="mt-1 block rounded border px-2 py-1">
          {children}
        </select>
      )}
    </Field>
  );
}
