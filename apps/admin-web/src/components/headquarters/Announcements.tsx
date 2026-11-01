"use client";

import { useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { Section } from "@/components/Section";

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
 * 一覧の各行は確かめてから取り下げ（withdrawAnnouncement）、取り下げられたらその行を応答の記録で置き換える。
 */
export function Announcements({
  api,
}: {
  api: Pick<HeadquartersApi, "sendAnnouncement" | "listAnnouncements" | "withdrawAnnouncement">;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);

  const fetchAnnouncements = useCallback(async (): Promise<State> => {
    const response = await api.listAnnouncements();
    return response.ok
      ? { kind: "loaded", announcements: response.data.announcements }
      : { kind: "failed", message: response.message };
  }, [api]);

  useEffect(() => {
    let active = true;
    void fetchAnnouncements().then((next) => {
      if (active) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchAnnouncements]);

  async function withdraw(announcement: Announcement) {
    const response = await api.withdrawAnnouncement(announcement.announcement_id);
    if (response.ok) {
      setResult({ ok: true, message: `お知らせ「${announcement.title}」を取り下げました。` });
      setState((current) =>
        current.kind === "loaded"
          ? {
              kind: "loaded",
              announcements: current.announcements.map((row) =>
                row.announcement_id === announcement.announcement_id ? response.data : row,
              ),
            }
          : current,
      );
      return;
    }
    // 409（すでに取り下げ済み）・404 などは ErrorCode の文言を出し、一覧を今の状態に取り直す。
    // 接続できなかったとき（code がない）は取り直さず、行はそのまま（もう一度押せる）。
    setResult(response);
    if (response.code !== undefined) setState(await fetchAnnouncements());
  }

  function prepend(announcement: Announcement) {
    setResult(null);
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
        <ResultMessage result={result} />
        {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
        {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
        {state.kind === "loaded" && state.announcements.length === 0 && (
          <p className="text-sm">まだお知らせを送っていません</p>
        )}
        {state.kind === "loaded" && state.announcements.length > 0 && (
          <AnnouncementList announcements={state.announcements} onWithdraw={withdraw} />
        )}
      </Section>
    </>
  );
}
