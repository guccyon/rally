"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { BookingList } from "./BookingList";

/** 本部の画面: 予約の一覧（日付ごとに、どの予約が入っていて、確定待ちのまま残っていないか）を見る。 */
export function BookingListHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="予約">
        <BookingList api={api} />
      </Section>
    </>
  );
}
