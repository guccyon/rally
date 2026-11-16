"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type ReactNode, useEffect } from "react";

import { Button } from "@/components/ui/Button";
import { ROLE_ENTRY_PATHS, ROLE_HOME_PATHS } from "@/session/rolePaths";
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
 * 指定のロールでログインしているときだけ中身を出す。ログインしていなければログイン画面へ戻す。
 * 別のロールでログイン済みなら、ログインのまま理由と自分のロールの画面への戻り道を出す（中身は出さない）。
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
    if (session === null) router.replace(ROLE_ENTRY_PATHS[role]);
  }, [session, router, role]);

  if (session && !allowed) {
    return (
      <div className="mx-auto max-w-[768px] px-4 py-6 min-[600px]:px-6">
        <h1 className="mb-3 text-headline font-black">この画面は{ROLE_LABELS[role]}のロールで開けます</h1>
        <p className="mb-4" role="alert">
          いまは{ROLE_LABELS[session.role]}のロールでログインしています。ログインしたままです。
        </p>
        <Link href={ROLE_HOME_PATHS[session.role]} className="text-primary underline">
          {ROLE_LABELS[session.role]}の画面へ戻る
        </Link>
      </div>
    );
  }

  if (!session) return <p className="p-6 text-text-muted">読み込み中…</p>;

  return (
    <div className={`mx-auto ${WIDTH_CLASSES[width]} px-4 py-6 min-[600px]:px-6`}>
      <header className="mb-6 flex items-center justify-between border-b pb-3">
        <h1 className="text-headline font-black">Rally 管理画面（{ROLE_LABELS[role]}）</h1>
        <Button
          variant="secondary"
          onClick={() => {
            clearSession();
            router.replace(ROLE_ENTRY_PATHS[role]);
          }}
        >
          ログアウト
        </Button>
      </header>
      {children(session)}
    </div>
  );
}
