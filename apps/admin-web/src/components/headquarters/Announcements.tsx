"use client";

import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { Section } from "@/components/ui/Section";

import { AnnouncementForm } from "./AnnouncementForm";
import { AnnouncementList } from "./AnnouncementList";

type Announcement = Schemas["Announcement"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; announcements: Announcement[] }
  | { kind: "failed"; message: string };

/**
 * 本部の「お知らせ」。上でお知らせを送り（sendAnnouncement）、下に送ったお知らせの一覧（listAnnouncements）を出す。
 * 一覧は開いたときに一度だけ読み、送れたら応答の記録を先頭に足す（取り直さない。契約の並びも新しい順）。
 */
export function Announcements({
  api,
}: {
  api: Pick<HeadquartersApi, "sendAnnouncement" | "listAnnouncements">;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let active = true;
    void api.listAnnouncements().then((response) => {
      if (!active) return;
      setState(
        response.ok
          ? { kind: "loaded", announcements: response.data.announcements }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api]);

  function prepend(announcement: Announcement) {
    setState((current) =>
      current.kind === "loaded"
        ? { kind: "loaded", announcements: [announcement, ...current.announcements] }
        : current,
    );
  }

  return (
    <>
      <Section title="お知らせを送る">
        <AnnouncementForm send={api.sendAnnouncement} onSent={prepend} />
      </Section>
      <Section title="送ったお知らせ">
        {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
        {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
        {state.kind === "loaded" && state.announcements.length === 0 && (
          <p className="text-sm">まだお知らせを送っていません</p>
        )}
        {state.kind === "loaded" && state.announcements.length > 0 && (
          <AnnouncementList announcements={state.announcements} />
        )}
      </Section>
    </>
  );
}
