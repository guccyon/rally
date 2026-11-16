"use client";

import { useRouter } from "next/navigation";
import { type ReactNode, useEffect } from "react";

import { ROLE_HOME_PATHS } from "@/session/rolePaths";
import { useSession } from "@/session/useSession";

/**
 * 管理画面の入口。ログインしたままならロールのホームへ置き換えて移り（入口を履歴に残さない）、
 * ログインしていないときだけ中身（ログインの画面）を出す。セッションを読むあいだと移るあいだは「読み込み中」。
 */
export function EntryGate({ children }: { children: ReactNode }) {
  const session = useSession();
  const router = useRouter();

  useEffect(() => {
    if (session) router.replace(ROLE_HOME_PATHS[session.role]);
  }, [session, router]);

  if (session !== null) return <p className="p-6 text-text-muted">読み込み中…</p>;
  return <>{children}</>;
}
