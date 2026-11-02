import type { ReactNode } from "react";

import { Icon } from "./Icon";

/**
 * 一覧の行（specs/design/components.yaml の list_row）。押すと詳細へ移る。
 * 未読は左に primary の 4px の線と、題の前の「未読」のしるしで示す（文字なので読み上げでも「未読」と読む）。
 */
export function ListRow({
  title,
  subtitle,
  trailing,
  unread = false,
  onClick,
}: {
  title: string;
  subtitle?: string;
  trailing?: ReactNode;
  unread?: boolean;
  onClick?: () => void;
}) {
  const body = (
    <>
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-2 font-bold">
          {unread && (
            <span className="rounded-sm bg-primary px-1 text-xs text-on-primary">未読</span>
          )}
          <span className="truncate">{title}</span>
        </span>
        {subtitle && <span className="block text-sm text-text-muted">{subtitle}</span>}
      </span>
      {trailing}
      {onClick && !trailing && <Icon name="chevron_right" size={24} className="text-text-muted" />}
    </>
  );
  const className = `flex min-h-14 w-full items-center gap-2 border-b px-4 py-2 text-left ${
    unread ? "border-l-4 border-l-primary" : ""
  }`;
  return onClick ? (
    <button type="button" onClick={onClick} className={`${className} active:bg-primary-container`}>
      {body}
    </button>
  ) : (
    <div className={className}>{body}</div>
  );
}
