"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { OverdueLiveSessionList } from "./OverdueLiveSessionList";

/** 本部の画面: 始まったまま終了の記録がなく、時間の過ぎた通話セッションをコーチを問わず見る。 */
export function OverdueLiveSessionsHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="終わっていない通話">
        <OverdueLiveSessionList api={api} />
      </Section>
    </>
  );
}
