// 読み込み中（specs/design/components.yaml の loading）。

/** 画面全体の読み込み。画面の上の 4px の bar。 */
export function LoadingBar({ label = "読み込んでいます" }: { label?: string }) {
  return (
    <div role="progressbar" aria-label={label} className="h-1 w-full overflow-hidden bg-primary-container">
      <div className="h-full w-1/3 bg-primary" />
    </div>
  );
}

/** 欄ごとの読み込み。「〜を読み込んでいます…」 */
export function SectionLoading({ message }: { message: string }) {
  return (
    <p role="status" className="text-sm text-text-muted">
      {message}
    </p>
  );
}

/** ボタンの中だけの 20px の spinner。 */
export function Spinner() {
  return (
    <span
      aria-hidden="true"
      data-testid="spinner"
      className="inline-block size-5 animate-spin rounded-pill border-2 border-current border-t-transparent"
    />
  );
}
