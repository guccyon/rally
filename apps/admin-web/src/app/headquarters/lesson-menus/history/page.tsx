"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  LESSON_MENU_ID_QUERY,
  LessonMenuPublicationHistoryHome,
} from "@/components/headquarters/LessonMenuPublicationHistoryHome";

function LessonMenuPublicationHistoryContent() {
  // レッスンメニューの一覧の行から `?lesson_menu_id=` で開く。
  const lessonMenuId = useSearchParams().get(LESSON_MENU_ID_QUERY) || undefined;
  return (
    <RoleGate role="headquarters">
      {(session) => (
        <LessonMenuPublicationHistoryHome token={session.token} lessonMenuId={lessonMenuId} />
      )}
    </RoleGate>
  );
}

export default function LessonMenuPublicationHistoryPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <LessonMenuPublicationHistoryContent />
    </Suspense>
  );
}
