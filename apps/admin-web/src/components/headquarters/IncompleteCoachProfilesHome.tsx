"use client";

import Link from "next/link";
import { useMemo } from "react";

import { createApiClient } from "@/api/client";
import { headquartersApi } from "@/api/headquarters";
import { Section } from "@/components/Section";

import { IncompleteCoachProfileList } from "./IncompleteCoachProfileList";

/** 本部の画面: プロフィール（自己紹介）に空の項目があるコーチを見る。 */
export function IncompleteCoachProfilesHome({ token }: { token: string }) {
  const api = useMemo(() => headquartersApi(createApiClient(token)), [token]);
  return (
    <>
      <p className="mb-4 text-sm">
        <Link href="/headquarters" className="text-blue-600 underline">
          本部の画面へ戻る
        </Link>
      </p>
      <Section title="自己紹介の足りないコーチ">
        <IncompleteCoachProfileList api={api} />
      </Section>
    </>
  );
}
