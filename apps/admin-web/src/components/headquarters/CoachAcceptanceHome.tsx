"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { CoachAcceptanceList } from "./CoachAcceptanceList";

/** 本部の画面: コーチの一覧で受付の状態を見て、受付を止める・再開する。 */
export function CoachAcceptanceHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="コーチの受付">
        <CoachAcceptanceList api={api} />
      </Section>
    </>
  );
}
