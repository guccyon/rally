"use client";

import { useSearchParams } from "next/navigation";
import { Suspense } from "react";

import { RoleGate } from "@/components/RoleGate";
import {
  LIVE_SESSION_RECORDS_STUDENT_QUERY,
  LiveSessionRecordsHome,
} from "@/components/headquarters/LiveSessionRecordsHome";

function LiveSessionRecordsContent() {
  // 生徒の利用の様子の画面から `?student_id=` で、その生徒で絞った状態で開く。
  const searchParams = useSearchParams();
  const studentId = searchParams.get(LIVE_SESSION_RECORDS_STUDENT_QUERY) || undefined;
  return (
    <RoleGate role="headquarters" width="wide">
      {(session) => <LiveSessionRecordsHome token={session.token} studentId={studentId} />}
    </RoleGate>
  );
}

export default function LiveSessionRecordsPage() {
  // useSearchParams はビルド時の静的な描画で中断するので、Suspense で囲む。
  return (
    <Suspense>
      <LiveSessionRecordsContent />
    </Suspense>
  );
}
