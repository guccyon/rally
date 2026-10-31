"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { StudentDirectory } from "./StudentDirectory";

/** 本部の画面: 生徒の一覧（どのスポーツの生徒が、どの契約で、最近予約しているか）を見る。 */
export function StudentDirectoryHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="生徒">
        <StudentDirectory api={api} />
      </Section>
    </>
  );
}
