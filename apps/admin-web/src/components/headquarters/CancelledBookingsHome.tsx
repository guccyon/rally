"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { CancelledBookingList } from "./CancelledBookingList";

/** 本部の画面: キャンセルされた予約を、キャンセルした日付の期間で見る（キャンセルが増えていないか・どのコーチの枠で多いか）。 */
export function CancelledBookingsHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="キャンセルされた予約">
        <CancelledBookingList api={api} />
      </Section>
    </>
  );
}
