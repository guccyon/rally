// 期間の誤り（specs/design/components.yaml の date_field の errors）。何が起きたかと、どうすればよいかを 1 つの文言にし、直す欄（印の欄）も返す。
// 日付は欄の値（YYYY-MM-DD）。日数は両端を含めて数える。

const DAY = 86_400_000;

/** 期間（開始日と終了日。欄の値の YYYY-MM-DD。空の欄は ""）。 */
export type DateRange = { from: string; to: string };

/** 印を付ける欄（errors の mark）。 */
export type DateRangeMark = "from" | "to" | "both";

export type DateRangeError = { message: string; mark: DateRangeMark };

/** 送るときに期間を確かめる。正しければ null。max は契約の期間の上限の日数。 */
export function dateRangeError({ from, to }: DateRange, max: number): DateRangeError | null {
  const fromDay = from === "" ? Number.NaN : Date.parse(from);
  const toDay = to === "" ? Number.NaN : Date.parse(to);
  if (Number.isNaN(fromDay) || Number.isNaN(toDay)) {
    const mark: DateRangeMark =
      Number.isNaN(fromDay) && Number.isNaN(toDay) ? "both" : Number.isNaN(fromDay) ? "from" : "to";
    return {
      message: "開始日か終了日が空いています。両方を選んでください。",
      mark,
    };
  }
  const days = (toDay - fromDay) / DAY + 1;
  if (days < 1) {
    return {
      message: "終了日が開始日より前です。終了日は開始日と同じか、それより後の日付にしてください。",
      mark: "to",
    };
  }
  if (days > max) {
    return {
      message: `選んだ期間は ${days} 日です。期間は ${max} 日までにしてください。`,
      mark: "both",
    };
  }
  return null;
}
