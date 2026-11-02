"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { SlotSupplyReport } from "./SlotSupplyReport";

/** 本部の画面: スポーツごとの予約枠の需給（どのスポーツのコーチを募集するかを見る）。 */
export function SlotSupplyHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="スポーツ別の予約枠の需給">
        <SlotSupplyReport api={api} />
      </Section>
    </>
  );
}
