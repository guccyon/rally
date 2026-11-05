"use client";

import { useRouter } from "next/navigation";
import { type ReactNode, useEffect } from "react";

import { clearSession, ROLE_LABELS, type Role, type Session } from "@/session/session";
import { useSession } from "@/session/useSession";

/**
 * 1 段の幅（specs/design/tokens.yaml の spacing.content_width_px）。reading は読む画面（最大 768px）、
 * wide は本部の一覧の画面（data_table が主役。最大 1280px）。wide でも表の外の文は 1 行 40em まで（spacing.wide_rules）。
 */
export type ContentWidth = "reading" | "wide";

const WIDTH_CLASSES: Record<ContentWidth, string> = {
  reading: "max-w-[768px]",
  wide: "max-w-[1280px] [&_p]:max-w-[40em]",
};

/**
 * 指定のロールでログインしているときだけ中身を出す。違えばログイン画面へ戻す。
 * 左右の余白は 600px 以上で lg 24px、600px 未満で md 16px（spacing.grid.admin_web_narrow）。
 */
export function RoleGate({
  role,
  width = "reading",
  children,
}: {
  role: Role;
  width?: ContentWidth;
  children: (session: Session) => ReactNode;
}) {
  const session = useSession();
  const router = useRouter();
  const allowed = session?.role === role;

  useEffect(() => {
    if (session !== undefined && !allowed) router.replace("/");
  }, [session, allowed, router]);

  if (!session || !allowed) return <p className="p-6 text-text-muted">読み込み中…</p>;

  return (
    <div className={`mx-auto ${WIDTH_CLASSES[width]} px-4 py-6 min-[600px]:px-6`}>
      <header className="mb-6 flex items-center justify-between border-b pb-3">
        <h1 className="text-headline font-black">Rally 管理画面（{ROLE_LABELS[role]}）</h1>
        <button
          type="button"
          className="rounded-pill border px-3 py-1 text-sm"
          onClick={() => {
            clearSession();
            router.replace("/");
          }}
        >
          ログアウト
        </button>
      </header>
      {children(session)}
    </div>
  );
}
