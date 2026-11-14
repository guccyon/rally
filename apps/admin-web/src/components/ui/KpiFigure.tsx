import { Card } from "./Card";
import { Icon } from "./Icon";

// 指標の値の組（specs/design/components.yaml の kpi_figure。0.9.0、#1492）。1 つの指標（specs/analytics/kpi.yaml）の
// 今月と前月の値を、単位・見方（良い向き）・出せないときの「—」と理由まで 1 組で出す。
// hero は北極星だけ（1 画面に 1 つ）。ほかの指標は DataTable の numeric の列に KpiCell を置く。
// 良い向きは値の色で示さず「見方」の文字で出す（trend_indicator を足すまで値の色は text のまま）。

/** 値の単位（kpi_figure.wording.number）。値は表示する単位のまま渡す（62.4% なら 62.4）。 */
export type KpiUnit = "percent" | "hours" | "none";

/** kpi.yaml の direction。 */
export type KpiDirection = "up_is_good" | "down_is_good";

/** 値を出せない理由（kpi_figure.wording.reasons）。not_yet は出る日（YYYY-MM-DD）を持つ。 */
export type KpiUnavailable = { reason: "too_few" } | { reason: "none" } | { reason: "not_yet"; availableOn: string };

/** 値か、出せない理由。出せない値を 0 や空欄にしない。 */
export type KpiValue = number | KpiUnavailable;

/** 出せない値の印（U+2014）。読み上げは「出せません」。 */
export const KPI_UNAVAILABLE_MARK = "—";

/** 見方の文字（kpi_figure.wording.direction）。 */
export const KPI_DIRECTION_LABELS: Record<KpiDirection, string> = {
  up_is_good: "高いほど良い",
  down_is_good: "低いほど良い",
};

/** hero の失敗の文（kpi_figure.wording.error）。 */
export const KPI_LOAD_ERROR = "今月の値を読めませんでした。画面を更新すると、もう一度読み込めます。";

const UNIT_TEXT: Record<KpiUnit, { digits: number; unit: string; separator: string }> = {
  percent: { digits: 1, unit: "%", separator: "" },
  hours: { digits: 1, unit: "時間", separator: " " },
  none: { digits: 3, unit: "", separator: "" },
};

/** 数字と単位に分ける（「62.4」と「%」）。今月と前月を同じ桁にそろえる。 */
export function kpiNumberParts(value: number, unit: KpiUnit): { number: string; unit: string; separator: string } {
  const text = UNIT_TEXT[unit];
  return { number: value.toFixed(text.digits), unit: text.unit, separator: text.separator };
}

/** 値を 1 つの文字にする（「62.4%」「18.5 時間」「0.012」）。 */
export function formatKpiNumber(value: number, unit: KpiUnit): string {
  const parts = kpiNumberParts(value, unit);
  return `${parts.number}${parts.separator}${parts.unit}`;
}

const WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];

/** YYYY-MM-DD を「11/30（月）」と「11/30」にする。形が違えばそのまま。 */
function shortDay(value: string): { withWeekday: string; plain: string } {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return { withWeekday: value, plain: value };
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  const weekday = WEEKDAYS[new Date(Date.UTC(year, month - 1, day)).getUTCDay()];
  return { withWeekday: `${month}/${day}（${weekday}）`, plain: `${month}/${day}` };
}

/** hero の理由の 1 文（「対象が 5 件未満のため、出せません。」）。 */
export function kpiReasonSentence(unavailable: KpiUnavailable): string {
  switch (unavailable.reason) {
    case "too_few":
      return "対象が 5 件未満のため、出せません。";
    case "none":
      return "対象が 0 件のため、出せません。";
    case "not_yet":
      return `集計の期間が終わっていないため、${shortDay(unavailable.availableOn).withWeekday}に出ます。`;
  }
}

/** cell の理由の短い語（10 字まで。「5 件未満」）。 */
export function kpiReasonShort(unavailable: KpiUnavailable): string {
  switch (unavailable.reason) {
    case "too_few":
      return "5 件未満";
    case "none":
      return "対象 0 件";
    case "not_yet":
      return `${shortDay(unavailable.availableOn).plain} に出ます`;
  }
}

/** 読み上げの値（出せないときは「出せません」）。 */
function spokenValue(value: KpiValue, unit: KpiUnit): string {
  return typeof value === "number" ? formatKpiNumber(value, unit) : "出せません";
}

/** hero を 1 文で読む（「初回レッスン後の 1 ヶ月継続率、今月 62.4%、前月 58.1%、高いほど良い」）。 */
export function kpiFigureSentence({
  label,
  unit,
  direction,
  current,
  previous,
}: {
  label: string;
  unit: KpiUnit;
  direction: KpiDirection;
  current: KpiValue;
  previous: KpiValue;
}): string {
  return [
    label,
    `今月 ${spokenValue(current, unit)}`,
    typeof current === "number" ? undefined : kpiReasonSentence(current).replace(/。$/, ""),
    `前月 ${spokenValue(previous, unit)}`,
    KPI_DIRECTION_LABELS[direction],
  ]
    .filter(Boolean)
    .join("、");
}

function Mark() {
  return <span aria-label="出せません">{KPI_UNAVAILABLE_MARK}</span>;
}

function previousText(previous: KpiValue, unit: KpiUnit): string {
  return `前月 ${typeof previous === "number" ? formatKpiNumber(previous, unit) : KPI_UNAVAILABLE_MARK}`;
}

/**
 * 北極星の値（kpi_figure の hero）。card の plain に、label（title）・今月の値（figure）と単位（title）・
 * 右に前月（small text_muted）・見方（small text_muted）を出す。1 画面に 1 つだけ置く。押せない部品。
 * 読んでいるあいだは loading（値の場所に「読み込み中…」。「—」を使わない）、読めなかったときは error（error_text）。
 */
export function KpiFigure({
  label,
  unit,
  direction,
  current,
  previous,
  loading,
  error,
}: {
  /** kpi.yaml の ja（「初回レッスン後の 1 ヶ月継続率」） */
  label: string;
  unit: KpiUnit;
  direction: KpiDirection;
  current?: KpiValue;
  previous?: KpiValue;
  loading?: boolean;
  /** 読めなかったときの文。既定は KPI_LOAD_ERROR */
  error?: boolean | string;
}) {
  const title = <p className="text-xl font-black">{label}</p>;
  if (loading || error || current === undefined || previous === undefined) {
    return (
      <Card>
        {title}
        {error ? (
          <p role="alert" className="flex items-start gap-1 text-sm text-danger">
            <Icon name="error" size={20} className="shrink-0" />
            {typeof error === "string" ? error : KPI_LOAD_ERROR}
          </p>
        ) : (
          <p role="status" className="text-sm text-text-muted">
            読み込み中…
          </p>
        )}
      </Card>
    );
  }
  const parts = typeof current === "number" ? kpiNumberParts(current, unit) : undefined;
  return (
    <div role="group" aria-label={kpiFigureSentence({ label, unit, direction, current, previous })}>
      <Card>
        {title}
        <div className="flex flex-wrap items-baseline gap-4">
          {parts ? (
            <span className="flex items-baseline gap-1 text-text">
              <span className="font-numeric text-figure font-bold tabular-nums">{parts.number}</span>
              {parts.unit && <span className="text-xl font-black">{parts.unit}</span>}
            </span>
          ) : (
            <span className="font-numeric text-figure font-bold text-text">
              <Mark />
            </span>
          )}
          <span className="text-sm text-text-muted tabular-nums">{previousText(previous, unit)}</span>
        </div>
        {typeof current !== "number" && <p className="text-sm text-text">{kpiReasonSentence(current)}</p>}
        <p className="text-sm text-text-muted">見方: {KPI_DIRECTION_LABELS[direction]}</p>
      </Card>
    </div>
  );
}

/**
 * 表のセルの値（kpi_figure の cell）。DataTable の numeric の列の cell に置く。
 * 前月の列は previous を付けて text_muted にする。出せないときは「—」の右に短い理由（text_muted）を 1 行で。
 */
export function KpiCell({ value, unit, previous = false }: { value: KpiValue; unit: KpiUnit; previous?: boolean }) {
  const color = previous ? "text-text-muted" : "text-text";
  if (typeof value === "number") return <span className={color}>{formatKpiNumber(value, unit)}</span>;
  return (
    <span className={color}>
      <Mark /> <span className="text-text-muted">{kpiReasonShort(value)}</span>
    </span>
  );
}
