"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/ui/Section";

import { LessonMenuNameHeading } from "./LessonMenuNameHeading";
import { LessonMenuPublicationHistory } from "./LessonMenuPublicationHistory";

/** URL でレッスンメニューを指すクエリの名前（`?lesson_menu_id=`）。 */
export const LESSON_MENU_ID_QUERY = "lesson_menu_id";

/** そのレッスンメニューの「公開の記録」の画面の URL。 */
export function lessonMenuPublicationHistoryPath(lessonMenuId: string): string {
  return `/headquarters/lesson-menus/history?${new URLSearchParams({ [LESSON_MENU_ID_QUERY]: lessonMenuId })}`;
}

/**
 * 本部の画面: レッスンメニューの一覧の行の「記録」から開き、メニュー 1 件を本部がいつ・なぜ非公開にし、
 * いつ公開に戻したかをたどる。記録の上にメニューの名前の見出しを出す（URL を共有したときもどのメニューか分かるように）。
 * メニューの ID がなければ見出しは出さない（記録の欄が「見つかりません」を出す）。
 */
export function LessonMenuPublicationHistoryHome({
  token,
  lessonMenuId,
}: {
  token: string;
  lessonMenuId: string | undefined;
}) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters/lesson-menus" className="text-primary underline">
          レッスンメニューの一覧へ戻る
        </Link>
      </p>
      <Section title="公開の記録">
        {lessonMenuId !== undefined && <LessonMenuNameHeading api={api} lessonMenuId={lessonMenuId} />}
        <LessonMenuPublicationHistory api={api} lessonMenuId={lessonMenuId} />
      </Section>
    </>
  );
}
