"use client";

import { useEffect, useState } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { Section } from "@/components/ui/Section";

import { AnnouncementDraftList } from "./AnnouncementDraftList";
import { AnnouncementForm } from "./AnnouncementForm";
import { AnnouncementList } from "./AnnouncementList";
import {
  type AnnouncementInput,
  EMPTY_INPUT,
  draftTitleOf,
  inputOfDraft,
  toAnnouncementDraftRequest,
} from "./announcementInput";

type Announcement = Schemas["Announcement"];
type AnnouncementDraft = Schemas["AnnouncementDraft"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; announcements: Announcement[] }
  | { kind: "failed"; message: string };

type DraftState =
  | { kind: "loading" }
  | { kind: "loaded"; drafts: AnnouncementDraft[] }
  | { kind: "failed"; message: string };

/** フォームに渡す中身。`key` を変えるとフォームを作り直す（下書きを開く・新しく書く）。 */
type FormSeed = { key: number; initial: AnnouncementInput; reviewOnOpen: boolean };

/**
 * 開いていた下書きがない（契約の 404 `not_found`。ほかの本部のアカウントが送ったか消した）ときの文言。汎用の
 * 「ID を確かめてください」では意味が通らないので、この画面で決める。
 */
const DRAFT_GONE =
  "この下書きは見つかりません。ほかの本部のアカウントが送ったか消した可能性があります。もう一度「下書きに保存」を押すと新しい下書きとして保存します。";
const DRAFT_GONE_ON_DELETE = "この下書きは見つかりません。ほかの本部のアカウントが送ったか消した可能性があります。";

export type AnnouncementsApi = Pick<
  HeadquartersApi,
  | "sendAnnouncement"
  | "listAnnouncements"
  | "createAnnouncementDraft"
  | "listAnnouncementDrafts"
  | "updateAnnouncementDraft"
  | "deleteAnnouncementDraft"
  | "sendAnnouncementDraft"
>;

/**
 * 本部の「お知らせ」。上でお知らせを書いて送るか下書きに保存し（sendAnnouncement / createAnnouncementDraft /
 * updateAnnouncementDraft）、その下に下書きの一覧（listAnnouncementDrafts）、いちばん下に送ったお知らせの一覧
 * （listAnnouncements）を出す。どちらの一覧も開いたときに一度だけ読み、操作の応答で手元を直す（取り直さない）。
 *
 * 下書きを開くとフォームに中身を入れ、「下書きに保存」はその下書きを直す。開いたまま送ると、書いた中身で下書きを直して
 * から下書きから送り（sendAnnouncementDraft）、送れたら下書きは一覧から消える。一覧の「送る」は開いてすぐに確かめに進む。
 */
export function Announcements({ api }: { api: AnnouncementsApi }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [drafts, setDrafts] = useState<DraftState>({ kind: "loading" });
  const [draftResult, setDraftResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [openedId, setOpenedId] = useState<string | null>(null);
  const [seed, setSeed] = useState<FormSeed>({ key: 0, initial: EMPTY_INPUT, reviewOnOpen: false });

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
    void api.listAnnouncementDrafts().then((response) => {
      if (!active) return;
      setDrafts(
        response.ok
          ? { kind: "loaded", drafts: response.data.announcement_drafts }
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

  /** 直した（作った）下書きを一覧の先頭に置く（最後に直した時刻の新しい順）。 */
  function putFirst(draft: AnnouncementDraft) {
    setDrafts((current) =>
      current.kind === "loaded"
        ? {
            kind: "loaded",
            drafts: [
              draft,
              ...current.drafts.filter((d) => d.announcement_draft_id !== draft.announcement_draft_id),
            ],
          }
        : current,
    );
  }

  function removeDraft(id: string) {
    setDrafts((current) =>
      current.kind === "loaded"
        ? { kind: "loaded", drafts: current.drafts.filter((d) => d.announcement_draft_id !== id) }
        : current,
    );
  }

  function reseed(initial: AnnouncementInput, reviewOnOpen: boolean) {
    setSeed((current) => ({ key: current.key + 1, initial, reviewOnOpen }));
  }

  function open(draft: AnnouncementDraft, reviewOnOpen: boolean) {
    setDraftResult(null);
    setOpenedId(draft.announcement_draft_id);
    reseed(inputOfDraft(draft), reviewOnOpen);
  }

  function startNew() {
    setOpenedId(null);
    reseed(EMPTY_INPUT, false);
  }

  async function saveDraft(
    request: Schemas["AnnouncementDraftRequest"],
  ): Promise<Result<AnnouncementDraft>> {
    if (openedId === null) {
      const created = await api.createAnnouncementDraft(request);
      if (created.ok) {
        setOpenedId(created.data.announcement_draft_id);
        putFirst(created.data);
      }
      return created;
    }
    const updated = await api.updateAnnouncementDraft(openedId, request);
    if (updated.ok) {
      putFirst(updated.data);
      return updated;
    }
    if (updated.code === "not_found") {
      removeDraft(openedId);
      setOpenedId(null);
      return { ok: false, message: DRAFT_GONE };
    }
    return updated;
  }

  async function send(request: Schemas["SendAnnouncementRequest"]): Promise<Result<Announcement>> {
    if (openedId === null) return api.sendAnnouncement(request);
    // 確かめた中身で送るように、先に下書きを直す
    const updated = await api.updateAnnouncementDraft(openedId, toAnnouncementDraftRequest(request));
    if (!updated.ok) {
      if (updated.code !== "not_found") return updated;
      removeDraft(openedId);
      setOpenedId(null);
      return { ok: false, message: DRAFT_GONE };
    }
    putFirst(updated.data);
    const sent = await api.sendAnnouncementDraft(openedId);
    if (sent.ok) {
      removeDraft(openedId);
      setOpenedId(null);
    }
    return sent;
  }

  async function deleteDraft(draft: AnnouncementDraft) {
    const id = draft.announcement_draft_id;
    const response = await api.deleteAnnouncementDraft(id);
    if (response.ok) {
      setDrafts({ kind: "loaded", drafts: response.data.announcement_drafts });
      setDraftResult({ ok: true, message: `下書き「${draftTitleOf(draft)}」を消しました。` });
    } else if (response.code === "not_found") {
      removeDraft(id);
      setDraftResult({ ok: false, message: DRAFT_GONE_ON_DELETE });
    } else {
      setDraftResult({ ok: false, message: response.message });
      return;
    }
    if (id === openedId) startNew();
  }

  const opened =
    openedId !== null && drafts.kind === "loaded"
      ? drafts.drafts.find((d) => d.announcement_draft_id === openedId)
      : undefined;

  return (
    <>
      <Section title="お知らせを送る">
        {openedId !== null && (
          <div className="mb-3 flex flex-wrap items-center gap-2 rounded bg-blue-50 p-2 text-sm">
            <span>
              下書き{opened ? `「${draftTitleOf(opened)}」` : ""}
              を開いています。送ると下書きは消え、送ったお知らせに移ります。
            </span>
            <button type="button" className="rounded border px-2 py-0.5" onClick={startNew}>
              新しく書く
            </button>
          </div>
        )}
        <AnnouncementForm
          key={seed.key}
          initial={seed.initial}
          reviewOnOpen={seed.reviewOnOpen}
          send={send}
          onSent={prepend}
          saveDraft={saveDraft}
        />
      </Section>
      <Section title="下書き">
        <p className="mb-3 text-xs text-gray-600">
          下書きは生徒に届きません（お知らせもプッシュ通知も送りません）。「送る」で確かめてから送ったときに初めて届きます。
        </p>
        <ResultMessage result={draftResult} />
        {drafts.kind === "loading" && <p className="text-sm text-gray-500">下書きを読み込み中…</p>}
        {drafts.kind === "failed" && <ResultMessage result={{ ok: false, message: drafts.message }} />}
        {drafts.kind === "loaded" && drafts.drafts.length === 0 && (
          <p className="text-sm">保存した下書きはありません</p>
        )}
        {drafts.kind === "loaded" && drafts.drafts.length > 0 && (
          <AnnouncementDraftList
            drafts={drafts.drafts}
            openedId={openedId}
            onOpen={(draft) => open(draft, false)}
            onSend={(draft) => open(draft, true)}
            onDelete={deleteDraft}
          />
        )}
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
