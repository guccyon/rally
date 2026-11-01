"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import { STUDENT_ID_QUERY, StudentActivityHome } from "@/components/headquarters/StudentActivityHome";

function StudentActivityContent() {
  // 生徒の一覧の行から `?student_id=` で開く。
  const studentId = useSearchParams().get(STUDENT_ID_QUERY) ?? undefined;
  return (
    <RoleGate role="headquarters">
      {(session) => <StudentActivityHome token={session.token} studentId={studentId} />}
    </RoleGate>
  );
}

export default function StudentActivityPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <StudentActivityContent />
    </Suspense>
  );
}
