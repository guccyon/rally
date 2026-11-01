import { ICON_PATHS, type IconName } from "./icons/paths";

/**
 * 同梱の Material Symbols Rounded のアイコン（icons/）。色は文字と同じ（currentColor）で、飾りなので読み上げない。
 * 意味は隣の文字か、押せる部品の aria-label で伝える。
 */
export function Icon({ name, size = 20, className }: { name: IconName; size?: 16 | 20 | 24; className?: string }) {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      data-icon={name}
      width={size}
      height={size}
      viewBox="0 -960 960 960"
      fill="currentColor"
      className={["inline-block shrink-0", className].filter(Boolean).join(" ")}
    >
      <path d={ICON_PATHS[name]} />
    </svg>
  );
}
