"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { LiveSessionRecordList } from "./LiveSessionRecordList";

/** 本部の画面: 問い合わせを受けたときに、ライブレッスン（通話セッション）がいつ始まりいつ終わったかを期間とコーチで引く。 */
export function LiveSessionRecordsHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="ライブレッスンの記録">
        <LiveSessionRecordList api={api} />
      </Section>
    </>
  );
}
