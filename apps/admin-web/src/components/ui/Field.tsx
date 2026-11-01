import { type ReactNode, useId } from "react";

export type FieldProps = {
  /** 項目の見出し（必ず付ける） */
  label: ReactNode;
  /** 送る前に分かる誤り。あれば invalid の状態にして項目の下に出す */
  error?: string;
  /** 項目の下の補足（残りの文字数など） */
  hint?: ReactNode;
};

/** 入力（components.yaml の input）の見出し・補足・誤りの並び。TextInput / TextArea / Select が使う。誤りは入力欄の aria-describedby から指す。 */
export function Field({
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
