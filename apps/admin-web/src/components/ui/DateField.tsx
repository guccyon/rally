import { useId, type InputHTMLAttributes } from "react";

import type { DateRange, DateRangeError, DateRangeMark } from "./dateRange";
import { Icon } from "./Icon";
import { TextField } from "./TextField";

// 日付の欄（specs/design/components.yaml の date_field）。画面に input type=date を直に書かず、この部品で描く。
// 欄の中は端末の日付の形で出し、値は YYYY-MM-DD（日本時間の日付として送る）。日付の選び方は端末のものを開く。

const DATE_INPUT = "min-w-[10em] tabular-nums";

type DateFieldProps = {
  label: string;
  helper?: string;
  error?: string;
} & Omit<InputHTMLAttributes<HTMLInputElement>, "type">;

/** 日付を 1 つ選ぶ欄（variant date）。見た目と誤りの出し方は text_field と同じ。 */
export function DateField({ className, ...props }: DateFieldProps) {
  return <TextField type="date" className={[DATE_INPUT, className].filter(Boolean).join(" ")} {...props} />;
}

type DateRangeFieldProps = {
  value: DateRange;
  onChange: (value: DateRange) => void;
  /** group の名前（既定「期間」） */
  legend?: string;
  /** 「31 日まで選べます」など */
  helper?: string;
  /** 送るときに確かめた誤り（dateRangeError）。印の欄だけ輪郭を danger にする */
  error?: DateRangeError | null;
  /** 既定の期間を API から読むあいだ。欄を変えられなくし、helper に「期間を読み込んでいます…」 */
  loading?: boolean;
};

const marked = (mark: DateRangeMark | undefined, field: "from" | "to") => mark === "both" || mark === field;

/** 期間を選ぶ欄（variant date_range）。開始日 〜 終了日を並べ、狭い幅では縦に並べる（〜 は出さない）。 */
export function DateRangeField({
  value,
  onChange,
  legend = "期間",
  helper,
  error,
  loading = false,
}: DateRangeFieldProps) {
  const id = useId();
  const helperId = `${id}-helper`;
  const errorId = `${id}-error`;
  const helperText = loading ? "期間を読み込んでいます…" : helper;
  const field = (name: "from" | "to") => {
    const invalid = error ? marked(error.mark, name) : false;
    return (
      <div className="min-w-0 flex-1">
        <DateField
          label={name === "from" ? "開始日" : "終了日"}
          value={value[name]}
          disabled={loading}
          onChange={(event) => onChange({ ...value, [name]: event.target.value })}
          aria-invalid={invalid || undefined}
          aria-describedby={[helperText && helperId, invalid && errorId].filter(Boolean).join(" ") || undefined}
        />
      </div>
    );
  };
  return (
    <fieldset className="m-0 min-w-0 border-0 p-0" aria-busy={loading || undefined}>
      <legend className="mb-2 p-0 text-sm font-bold text-text">{legend}</legend>
      <div className="flex flex-col gap-2 min-[600px]:flex-row min-[600px]:items-end">
        {field("from")}
        <span aria-hidden="true" className="hidden min-h-12 items-center text-base text-text-muted min-[600px]:flex">
          〜
        </span>
        {field("to")}
      </div>
      {helperText && (
        <p id={helperId} role={loading ? "status" : undefined} className="mt-2 text-sm text-text-muted">
          {helperText}
        </p>
      )}
      {error && (
        <p id={errorId} role="alert" className="mt-2 flex items-start gap-1 text-sm text-danger">
          <Icon name="error" size={20} />
          {error.message}
        </p>
      )}
    </fieldset>
  );
}
