"use client";

import { useRouter } from "next/navigation";
import { type ReactNode, useEffect } from "react";

import { clearSession, ROLE_LABELS, type Role, type Session } from "@/session/session";
import { useSession } from "@/session/useSession";

/** 指定のロールでログインしているときだけ中身を出す。違えばログイン画面へ戻す。 */
export function RoleGate({
  role,
  children,
}: {
  role: Role;
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
    <div className="mx-auto max-w-3xl p-6">
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
