"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { LessonMenuCatalog } from "./LessonMenuCatalog";

/** URL でコーチを絞った状態にするクエリの名前（`?coach_id=`）。 */
export const LESSON_MENU_COACH_QUERY = "coach_id";

/** そのコーチで絞った「レッスンメニュー」の画面の URL。 */
export function lessonMenuCatalogPath(coachId: string): string {
  return `/headquarters/lesson-menus?${new URLSearchParams({ [LESSON_MENU_COACH_QUERY]: coachId })}`;
}

/**
 * 本部の画面: コーチのレッスンメニューを横断で見る（どのスポーツにどんなメニューがあるかを把握する）。
 * `initialCoachId`（URL の `?coach_id=`）があれば、そのコーチで絞った状態で開く。
 */
export function LessonMenuCatalogHome({
  token,
  initialCoachId,
}: {
  token: string;
  initialCoachId?: string;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="レッスンメニュー">
        <LessonMenuCatalog api={api} initialCoachId={initialCoachId} />
      </Section>
    </>
  );
}
