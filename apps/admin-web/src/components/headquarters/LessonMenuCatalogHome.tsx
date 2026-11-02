"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { LessonMenuCatalog, type LessonMenuCatalogQuery } from "./LessonMenuCatalog";

/** 「レッスンメニュー」の画面の URL。 */
const LESSON_MENU_CATALOG_PATH = "/headquarters/lesson-menus";

/** URL でコーチを絞った状態にするクエリの名前（`?coach_id=`）。 */
export const LESSON_MENU_COACH_QUERY = "coach_id";

/** URL に名前の検索語を残すクエリの名前（`?q=`。契約の `listLessonMenus` のクエリと同じ）。 */
export const LESSON_MENU_NAME_QUERY = "q";

/** コーチと名前の検索語を残した「レッスンメニュー」の画面の URL。空の条件はクエリに出さない。 */
export function lessonMenuCatalogQueryPath(query: LessonMenuCatalogQuery): string {
  const params = new URLSearchParams();
  if (query.coachId !== "") params.set(LESSON_MENU_COACH_QUERY, query.coachId);
  if (query.q !== "") params.set(LESSON_MENU_NAME_QUERY, query.q);
  const search = params.toString();
  return search === "" ? LESSON_MENU_CATALOG_PATH : `${LESSON_MENU_CATALOG_PATH}?${search}`;
}

/** そのコーチで絞った「レッスンメニュー」の画面の URL。 */
export function lessonMenuCatalogPath(coachId: string): string {
  return lessonMenuCatalogQueryPath({ coachId, q: "" });
}

/**
 * 本部の画面: コーチのレッスンメニューを横断で見る（どのスポーツにどんなメニューがあるかを把握する）。
 * `initialCoachId`（URL の `?coach_id=`）があればそのコーチで、`initialQuery`（URL の `?q=`）があればその名前で絞った状態で開く。
 * コーチと名前の検索語は URL のクエリに残し、再読み込み・共有で同じ一覧を開く。
 */
export function LessonMenuCatalogHome({
  token,
  initialCoachId,
  initialQuery,
}: {
  token: string;
  initialCoachId?: string;
  initialQuery?: string;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  const router = useRouter();
  // 絞り込みを変えるたびに履歴を積まず URL だけを置き換える（戻るで条件を 1 つずつ戻さない）。
  const keepInUrl = useCallback(
    (query: LessonMenuCatalogQuery) =>
      router.replace(lessonMenuCatalogQueryPath(query), { scroll: false }),
    [router],
  );
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-primary underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="レッスンメニュー">
        <LessonMenuCatalog
          api={api}
          initialCoachId={initialCoachId}
          initialQuery={initialQuery}
          onQueryChange={keepInUrl}
        />
      </Section>
    </>
  );
}
