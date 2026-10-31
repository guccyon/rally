"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { LessonMenuCatalog } from "./LessonMenuCatalog";

/** 本部の画面: コーチのレッスンメニューを横断で見る（どのスポーツにどんなメニューがあるかを把握する）。 */
export function LessonMenuCatalogHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="レッスンメニュー">
        <LessonMenuCatalog api={api} />
      </Section>
    </>
  );
}
