import { useId, type SelectHTMLAttributes } from "react";

import { Icon } from "./Icon";
import { FIELD, FieldFrame, describedBy } from "./TextField";

// 選択欄（specs/design/components.yaml の select）。画面に select 要素を直に書かず、この部品で描く。
// 端末の select 要素をそのまま使い（選び方のリストは端末に任せる）、形は text_field の field と同じにする。
// 先頭の行（絞り込みは「すべて」、必須の欄は「選んでください」）と選択肢の順は呼ぶ側が children の option で渡す。

const SELECT = "peer min-h-12 appearance-none truncate pr-10";

type SelectProps = {
  /** 名詞のラベル（「コーチ」「スポーツ」）。誤りと読み込みの文言の {名前} にも使う */
  label: string;
  /** 押せない理由など（「お知らせを送っているあいだは変えられません。」） */
  helper?: string;
  /** 送るときに確かめた誤り（selectNotSelected）。輪郭を danger にする */
  error?: string;
  /** 選択肢を API から読むあいだ。欄を変えられなくし、helper に「〜の一覧を読み込んでいます…」 */
  loading?: boolean;
  /** 選択肢を読めなかったとき。欄を変えられなくし、下に読めなかったことと読み直し方を出す（輪郭は変えない） */
  loadFailed?: boolean;
  /** form の 1 段では幅いっぱい。絞り込みの行では中身で決める（10em〜20em） */
  fullWidth?: boolean;
} & SelectHTMLAttributes<HTMLSelectElement>;

/** 選択肢を読めなかったときの文言（errors の load_failed）。 */
export function selectLoadFailed(name: string): string {
  return `${name}の一覧を読めませんでした。画面を更新すると、もう一度読み込めます。`;
}

/** 必須の欄を選んでいないときの文言（errors の not_selected）。 */
export function selectNotSelected(name: string): string {
  return `${name}が選ばれていません。一覧から 1 つ選んでください。`;
}

/** 決まった選択肢から 1 つを選ぶ欄。高さ 48px・右に expand_more の chevron。 */
export function Select({
  label,
  helper,
  error,
  loading = false,
  loadFailed = false,
  fullWidth = false,
  disabled,
  className,
  children,
  ...props
}: SelectProps) {
  const id = useId();
  const helperText = loading ? `${label}の一覧を読み込んでいます…` : helper;
  const errorText = loadFailed ? selectLoadFailed(label) : error;
  return (
    <FieldFrame
      id={id}
      label={label}
      helper={helperText}
      error={errorText}
      helperRole={loading ? "status" : undefined}
      errorRole={loadFailed ? "alert" : undefined}
      className={fullWidth ? undefined : "min-w-[10em] max-w-[20em]"}
    >
      <div className="relative">
        <select
          id={id}
          disabled={disabled || loading || loadFailed}
          aria-busy={loading || undefined}
          aria-invalid={error && !loadFailed ? true : undefined}
          aria-describedby={describedBy(id, { label, helper: helperText, error: errorText })}
          className={[FIELD, SELECT, className].filter(Boolean).join(" ")}
          {...props}
        >
          {children}
        </select>
        <Icon
          name="expand_more"
          size={20}
          className="pointer-events-none absolute top-1/2 right-4 -translate-y-1/2 text-text peer-disabled:text-text-muted"
        />
      </div>
    </FieldFrame>
  );
}
