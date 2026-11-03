"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { OverdueLiveSessionList } from "./OverdueLiveSessionList";

/** 「終わっていない通話」の画面の URL。 */
export const OVERDUE_LIVE_SESSIONS_PATH = "/headquarters/overdue-live-sessions";

/** URL でコーチを絞った状態にするクエリの名前（契約の `listOverdueLiveLessonSessions` のクエリと同じ `?coach_id=`）。 */
export const OVERDUE_LIVE_SESSIONS_COACH_QUERY = "coach_id";

/** コーチで絞った「終わっていない通話」の画面の URL。「すべて」（空）はクエリに出さない。 */
export function overdueLiveSessionsPath(coachId: string): string {
  return coachId === ""
    ? OVERDUE_LIVE_SESSIONS_PATH
    : `${OVERDUE_LIVE_SESSIONS_PATH}?${new URLSearchParams({ [OVERDUE_LIVE_SESSIONS_COACH_QUERY]: coachId })}`;
}

/**
 * 本部の画面: 始まったまま終了の記録がなく、時間の過ぎた通話セッションを見る。コーチの絞り込みは URL のクエリ
 * （`?coach_id=`）に残し、再読み込み・共有で同じコーチの通話を開く。
 */
export function OverdueLiveSessionsHome({ token, initialCoachId }: { token: string; initialCoachId?: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (coachId: string) => router.replace(overdueLiveSessionsPath(coachId), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="終わっていない通話">
        <OverdueLiveSessionList api={api} initialCoachId={initialCoachId} onCoachChange={keepInUrl} />
      </Section>
    </>
  );
}
