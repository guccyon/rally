import type { IconName } from "./icons/paths";

// 状態の色（specs/design/tokens.yaml の color.status と booking_state の neutral）。
// 状態は色と一緒に必ずアイコンか文字を出す。

export type Status = "info" | "success" | "warning" | "danger" | "neutral";

/** 面と、面の上の文字（status_tag・notice の inline）。 */
export const STATUS_CONTAINER_CLASSES: Record<Status, string> = {
  info: "bg-info-container text-info-on-container",
  success: "bg-success-container text-success-on-container",
  warning: "bg-warning-container text-warning-on-container",
  danger: "bg-danger-container text-danger-on-container",
  neutral: "bg-surface text-text-muted",
};

/** アイコンの色（status の fg）。 */
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
