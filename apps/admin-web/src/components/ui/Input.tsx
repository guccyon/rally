import {
  type InputHTMLAttributes,
  type ReactNode,
  type SelectHTMLAttributes,
  type TextareaHTMLAttributes,
  useId,
} from "react";

type FieldProps = {
  /** 項目の見出し（必ず付ける） */
  label: ReactNode;
  /** 送る前に分かる誤り。あれば invalid の状態にして項目の下に出す */
  error?: string;
  /** 項目の下の補足（残りの文字数など） */
  hint?: ReactNode;
};

/** 見出し・補足・誤りの並び。誤りは入力欄の aria-describedby から指す。 */
function Field({
  label,
  error,
  hint,
  children,
}: FieldProps & { children: (a11y: { "aria-invalid"?: true; "aria-describedby"?: string }) => ReactNode }) {
  const errorId = useId();
  return (
    <div>
      <label className="block">
        {label}
        {children(error ? { "aria-invalid": true, "aria-describedby": errorId } : {})}
      </label>
      {hint}
      {error && (
        <p id={errorId} className="text-sm text-red-600">
          {error}
        </p>
      )}
    </div>
  );
}

type TextInputProps = FieldProps & Omit<InputHTMLAttributes<HTMLInputElement>, "className">;

/** 1 行の入力（components.yaml の input / single_line）。 */
export function TextInput({ label, error, hint, type = "text", ...rest }: TextInputProps) {
  return (
    <Field label={label} error={error} hint={hint}>
      {(a11y) => <input {...rest} {...a11y} type={type} className="mt-1 block w-full rounded border px-2 py-1" />}
    </Field>
  );
}

type TextAreaProps = FieldProps & Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className">;

/** 複数行の入力（components.yaml の input / multi_line）。 */
export function TextArea({ label, error, hint, ...rest }: TextAreaProps) {
  return (
    <Field label={label} error={error} hint={hint}>
      {(a11y) => <textarea {...rest} {...a11y} className="mt-1 block w-full rounded border px-2 py-1" />}
    </Field>
  );
}

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
