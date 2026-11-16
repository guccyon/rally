import type { IconName } from "./icons/paths";

// 状態の色（specs/design/tokens.yaml の color.status と booking_state・slot_state の neutral）。
// 状態は色の点かアイコンと一緒に、必ず glossary の呼び名の文字を出す。container の面の札と知らせは使わない（0.5.0）。

export type Status = "info" | "success" | "warning" | "danger" | "neutral";

/** 状態の点（status_tag）。status の fg、neutral は border-strong。 */
export const STATUS_DOT_CLASSES: Record<Status, string> = {
  info: "bg-info",
  success: "bg-success",
  warning: "bg-warning",
  danger: "bg-danger",
  neutral: "bg-border-strong",
};

/** notice の inline の左の 4px の線（status の fg）。 */
export const STATUS_EDGE_CLASSES: Record<Status, string> = {
  info: "border-l-info",
  success: "border-l-success",
  warning: "border-l-warning",
  danger: "border-l-danger",
  neutral: "border-l-border-strong",
};

/** アイコンの色（status の fg）。neutral は text-muted。 */
export const STATUS_ICON_CLASSES: Record<Status, string> = {
  info: "text-info",
  success: "text-success",
  warning: "text-warning",
  danger: "text-danger",
  neutral: "text-text-muted",
};

/** 状態のアイコン（icons/ の Material Symbols Rounded）。 */
export const STATUS_ICONS: Record<Status, IconName> = {
  info: "info",
  success: "check_circle",
  warning: "schedule",
  danger: "error",
  neutral: "do_not_disturb_on",
};

/** スロットの状態（glossary の slot.states）と色の対応（tokens.yaml の color.slot_state）。 */
export const SLOT_STATE_STATUS: Record<"open" | "booked" | "closed", Status> = {
  open: "info",
  booked: "success",
  closed: "neutral",
};

/**
 * 通話品質の状態と色の対応（specs/design/components.yaml の status_tag.call_quality_state）。呼び名は glossary の
 * call_quality_score.states。danger は low だけで、値を足すときは warning か neutral にする（call_quality_rule）。
 * キーは #1465 の契約で決まるまで仮。
 */
export const CALL_QUALITY_STATE_STATUS: Record<"low" | "fair" | "ok" | "no_data", Status> = {
  low: "danger",
  fair: "warning",
  ok: "neutral",
  no_data: "neutral",
};
