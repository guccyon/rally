"use client";

import { type FormEvent, useCallback, useEffect, useRef, useState } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { SPORT_LABELS, formatDay, toJstDate } from "@/components/labels";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { Section } from "@/components/ui/Section";

import { AnnouncementDraftList } from "./AnnouncementDraftList";
import { AnnouncementForm } from "./AnnouncementForm";
import { AnnouncementList } from "./AnnouncementList";
import {
  type AnnouncementInput,
  DRAFT_QUERY_TOO_LONG,
  EMPTY_INPUT,
  draftTitleOf,
  draftTitleQuery,
  inputOfDraft,
  toAnnouncementDraftRequest,
} from "./announcementInput";
import { MAX_PERIOD_DAYS, type Period, periodError, recentPeriod } from "./bookingPeriod";
import { SPORTS } from "./PendingVideoReviewList";

type Announcement = Schemas["Announcement"];
type AnnouncementDraft = Schemas["AnnouncementDraft"];
type Sport = Schemas["Sport"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; announcements: Announcement[]; period: Period; q: string; sport: Sport | "" }
  | { kind: "failed"; message: string };

type DraftState =
  | { kind: "loading" }
  | { kind: "loaded"; drafts: AnnouncementDraft[]; q: string }
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

// 送る前の確かめ（periodError）をすり抜けた 422 の理由。サーバーの `message` は表示用ではない（契約）ので、直す場所をここで伝える。
const VALIDATION_FAILED = `期間の指定が正しくありません。期間は 1〜${MAX_PERIOD_DAYS} 日で、終了日は開始日と同じかそれより後の日付にしてください。`;

/** 送ったお知らせの一覧を読む条件（期間・確定した題の語・スポーツ）。語とスポーツは空なら絞らない。 */
type SentFilter = { period: Period; q: string; sport: Sport | "" };

/**
 * 0 件のときの文言。スポーツで絞っていればその競技の生徒に向けて送ったもの、題で探していれば語を含むもののことだと言う。
 * スポーツで絞ったときは生徒全員に向けて送ったものは載らない（契約）ので、「〜の生徒に向けて送った」と言う。
 */
function emptyMessageOf({ q, sport }: SentFilter): string {
  const sent = sport === "" ? "この期間に送った" : `この期間に${SPORT_LABELS[sport]}の生徒に向けて送った`;
  return q === "" ? `${sent}お知らせはありません` : `${sent}、題に「${q}」を含むお知らせはありません`;
}

/** 送った日付（日本時間）が期間に入るか。期間の外で送ったお知らせは、今の一覧に足さない。 */
function sentWithin(announcement: Announcement, { from, to }: Period): boolean {
  const day = toJstDate(announcement.sent_at);
  return from <= day && day <= to;
}

export type AnnouncementsApi = Pick<
  HeadquartersApi,
  | "sendAnnouncement"
  | "listAnnouncements"
  | "withdrawAnnouncement"
  | "createAnnouncementDraft"
  | "listAnnouncementDrafts"
  | "updateAnnouncementDraft"
  | "deleteAnnouncementDraft"
  | "sendAnnouncementDraft"
>;

/**
 * 本部の「お知らせ」。上でお知らせを書いて送るか下書きに保存し（sendAnnouncement / createAnnouncementDraft /
 * updateAnnouncementDraft）、その下に下書きの一覧（listAnnouncementDrafts）、いちばん下に送ったお知らせの一覧
 * （listAnnouncements）を出す。どちらの一覧も開いたときと絞り込み（期間・題）を変えたときに読み、操作の応答で手元を直す
 * （取り直さない。題で絞っている間の下書きの保存だけは下の例外）。
 * 送ったお知らせの一覧は送った日付（日本時間）の期間で絞る。期間の入れ方・既定（今日までの 7 日）・上限（31 日）は
 * キャンセルされた予約の一覧と同じで、「この期間で見る」で読み直す。送ったお知らせは期間に入るときだけ一覧の先頭に足す。
 * 送ったお知らせの各行は確かめてから取り下げ（withdrawAnnouncement）、取り下げられたらその行を応答の記録で置き換える
 * （断られたら送ったお知らせの一覧だけを取り直す）。
 * 送ったお知らせの一覧の「題で探す」は下書きの一覧の検索と同じ部品・同じ確かめで、listAnnouncements の `q` に渡す。
 * 期間と題はそれぞれのボタンで確定し、確定したもう一方の条件と組み合わせて読み直す。題で絞っている間に送ったお知らせは、
 * 題が語に合うかを画面で決めず、今の期間と語で一覧を読み直す。
 * 「スポーツ」はほかの一覧のスポーツの欄と同じ部品で、選んだらすぐに listAnnouncements の `sport` に渡し、確定した期間・題と
 * 組み合わせて読み直す（「すべて」なら絞らない）。スポーツで絞っている間に送ったお知らせも、載るかを画面で決めず今の条件で読み直す。
 *
 * 下書きを開くとフォームに中身を入れ、「下書きに保存」はその下書きを直す。開いたまま送ると、書いた中身で下書きを直して
 * から下書きから送り（sendAnnouncementDraft）、送れたら下書きは一覧から消える。一覧の「送る」は開いてすぐに確かめに進む。
 *
 * 下書きの一覧の上の「題で探す」は、ほかの一覧の名前の検索と同じく「探す」（または Enter）で確定し、前後の空白を除いた語を
 * listAnnouncementDrafts の `q` に渡して読み直す（空なら絞らない）。上限を超える語は送らずに理由を出す。
 * 題で絞っている間に保存した下書きは、題が語に合うかを画面で決めず（一致のしかたは契約が正本）、今の語で一覧を読み直す。
 */
export function Announcements({ api }: { api: AnnouncementsApi }) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [drafts, setDrafts] = useState<DraftState>({ kind: "loading" });
  const [draftResult, setDraftResult] = useState<{ ok: boolean; message: string } | null>(null);
  // 下書きの題の検索欄の入力中の値と、今の下書きの一覧を読んだ語（確定した語。空なら絞らない）。
  const [draftQueryDraft, setDraftQueryDraft] = useState("");
  const [draftQueryError, setDraftQueryError] = useState<string | null>(null);
  const appliedDraftQuery = useRef("");
  // 語を続けて変えたときに、前の語の遅れて返った結果で上書きしないための番号。
  const latestDraftRequest = useRef(0);
  const [openedId, setOpenedId] = useState<string | null>(null);
  const [seed, setSeed] = useState<FormSeed>({ key: 0, initial: EMPTY_INPUT, reviewOnOpen: false });
  // 期間の欄の値（draftPeriod）と、今の一覧を読んだ期間（appliedPeriod。取り下げの後の取り直しにも使う）。
  const [draftPeriod, setDraftPeriod] = useState<Period>(() => recentPeriod(Date.now()));
  const [periodDraftError, setPeriodDraftError] = useState<string | null>(null);
  const appliedPeriod = useRef(draftPeriod);
  // 送ったお知らせの題の検索欄の入力中の値と、今の一覧を読んだ語（確定した語。空なら絞らない）。
  const [queryDraft, setQueryDraft] = useState("");
  const [queryError, setQueryError] = useState<string | null>(null);
  const appliedQuery = useRef("");
  // 送ったお知らせを絞るスポーツ（選んだらすぐ読み直すので、欄の値と今の一覧を読んだスポーツは同じ。空なら絞らない）。
  const [sport, setSport] = useState<Sport | "">("");
  const appliedSport = useRef<Sport | "">("");
  // 期間を続けて変えたときに、前の期間の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchAnnouncements = useCallback(
    async ({ period, q, sport }: SentFilter): Promise<State> => {
      const response = await api.listAnnouncements({
        ...period,
        ...(q === "" ? {} : { q }),
        ...(sport === "" ? {} : { sport }),
      });
      if (response.ok) return { kind: "loaded", announcements: response.data.announcements, period, q, sport };
      if (response.code !== "validation_failed") return { kind: "failed", message: response.message };
      // 期間も語も送る前に確かめているので、422 はふつう来ない。来たら直す場所（期間と、語で探していれば語の長さ）を添える。
      return { kind: "failed", message: q === "" ? VALIDATION_FAILED : `${VALIDATION_FAILED}${DRAFT_QUERY_TOO_LONG}` };
    },
    [api],
  );

  /** 期間・語・スポーツで送ったお知らせの一覧を読み直す（後から読み直したときは、前の遅れた結果を捨てる）。 */
  const reload = useCallback(
    async (filter: SentFilter) => {
      const request = ++latestRequest.current;
      const next = await fetchAnnouncements(filter);
      if (request === latestRequest.current) setState(next);
    },
    [fetchAnnouncements],
  );

  const fetchDrafts = useCallback(
    async (q: string): Promise<DraftState> => {
      const response = await api.listAnnouncementDrafts(q === "" ? {} : { q });
      if (response.ok) return { kind: "loaded", drafts: response.data.announcement_drafts, q };
      // 検索欄からは送る前に確かめているので、422 はふつう来ない。来たら直す場所（語の長さ）を添える。
      return {
        kind: "failed",
        message: response.code === "validation_failed" ? `${response.message}${DRAFT_QUERY_TOO_LONG}` : response.message,
      };
    },
    [api],
  );

  /** 語 q で下書きの一覧を読み直す（後から読み直したときは、前の遅れた結果を捨てる）。 */
  const reloadDrafts = useCallback(
    async (q: string) => {
      const request = ++latestDraftRequest.current;
      const next = await fetchDrafts(q);
      if (request === latestDraftRequest.current) setDrafts(next);
    },
    [fetchDrafts],
  );

  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchAnnouncements({
      period: appliedPeriod.current,
      q: appliedQuery.current,
      sport: appliedSport.current,
    }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    const draftRequest = ++latestDraftRequest.current;
    void fetchDrafts(appliedDraftQuery.current).then((next) => {
      if (active && draftRequest === latestDraftRequest.current) setDrafts(next);
    });
    return () => {
      active = false;
    };
  }, [fetchAnnouncements, fetchDrafts]);

  /** 確定した期間・語・スポーツ（今の一覧を読んだ条件）。 */
  function appliedFilter(): SentFilter {
    return { period: appliedPeriod.current, q: appliedQuery.current, sport: appliedSport.current };
  }

  async function withdraw(announcement: Announcement) {
    const response = await api.withdrawAnnouncement(announcement.announcement_id);
    if (response.ok) {
      setResult({ ok: true, message: `お知らせ「${announcement.title}」を取り下げました。` });
      setState((current) =>
        current.kind === "loaded"
          ? {
              ...current,
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
    if (response.code !== undefined) await reload(appliedFilter());
  }

  function prepend(announcement: Announcement) {
    setResult(null);
    if (appliedQuery.current !== "" || appliedSport.current !== "") {
      void reload(appliedFilter());
      return;
    }
    setState((current) =>
      current.kind === "loaded" && sentWithin(announcement, current.period)
        ? { ...current, announcements: [announcement, ...current.announcements] }
        : current,
    );
  }

  async function submitPeriod(event: FormEvent) {
    event.preventDefault();
    const error = periodError(draftPeriod);
    setPeriodDraftError(error);
    if (error !== null) return;
    appliedPeriod.current = draftPeriod;
    setResult(null);
    setState({ kind: "loading" });
    await reload({ ...appliedFilter(), period: draftPeriod });
  }

  async function submitQuery(event: FormEvent) {
    event.preventDefault();
    const query = draftTitleQuery(queryDraft);
    if (!query.ok) {
      setQueryError(query.message);
      return;
    }
    setQueryError(null);
    setQueryDraft(query.q);
    appliedQuery.current = query.q;
    setResult(null);
    setState({ kind: "loading" });
    await reload({ ...appliedFilter(), q: query.q });
  }

  async function changeSport(next: Sport | "") {
    setSport(next);
    appliedSport.current = next;
    setResult(null);
    setState({ kind: "loading" });
    await reload({ ...appliedFilter(), sport: next });
  }

  async function submitDraftQuery(event: FormEvent) {
    event.preventDefault();
    const query = draftTitleQuery(draftQueryDraft);
    if (!query.ok) {
      setDraftQueryError(query.message);
      return;
    }
    setDraftQueryError(null);
    setDraftQueryDraft(query.q);
    appliedDraftQuery.current = query.q;
    setDraftResult(null);
    setDrafts({ kind: "loading" });
    await reloadDrafts(query.q);
  }

  /**
   * 直した（作った）下書きを一覧の先頭に置く（最後に直した時刻の新しい順）。題で絞っているときは、題が語に合うかを
   * 画面で決めずに今の語で読み直す。
   */
  function putFirst(draft: AnnouncementDraft) {
    if (appliedDraftQuery.current !== "") {
      void reloadDrafts(appliedDraftQuery.current);
      return;
    }
    setDrafts((current) =>
      current.kind === "loaded"
        ? {
            ...current,
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
        ? { ...current, drafts: current.drafts.filter((d) => d.announcement_draft_id !== id) }
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
    // 題で絞っているときは、送り終わってから一覧を直す（送る前に読み直すと、送った下書きが遅れて一覧に戻りうる）
    const filtered = appliedDraftQuery.current !== "";
    if (!filtered) putFirst(updated.data);
    const sent = await api.sendAnnouncementDraft(openedId);
    if (sent.ok) {
      removeDraft(openedId);
      setOpenedId(null);
    } else if (filtered) {
      putFirst(updated.data);
    }
    return sent;
  }

  async function deleteDraft(draft: AnnouncementDraft) {
    const id = draft.announcement_draft_id;
    const response = await api.deleteAnnouncementDraft(id);
    if (response.ok) {
      // 応答は絞らない残りの一覧なので、題で絞っているときは消した行だけを外す
      if (appliedDraftQuery.current === "") {
        setDrafts({ kind: "loaded", drafts: response.data.announcement_drafts, q: "" });
      } else {
        removeDraft(id);
      }
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
          <div className="mb-3 flex flex-wrap items-center gap-2 rounded-md bg-primary-container p-2 text-sm">
            <span>
              下書き{opened ? `「${draftTitleOf(opened)}」` : ""}
              を開いています。送ると下書きは消え、送ったお知らせに移ります。
            </span>
            <button type="button" className="rounded-pill border px-2 py-0.5" onClick={startNew}>
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
        <p className="mb-3 text-xs text-text-muted">
          下書きは生徒に届きません（お知らせもプッシュ通知も送りません）。「送る」で確かめてから送ったときに初めて届きます。
        </p>
        <form className="mb-3 flex items-end gap-2" onSubmit={(event) => void submitDraftQuery(event)}>
          <label className="block">
            題で探す
            <input
              type="search"
              className="mt-1 block rounded-md border px-2 py-1"
              value={draftQueryDraft}
              onChange={(event) => setDraftQueryDraft(event.target.value)}
            />
          </label>
          <button type="submit" className="rounded-pill bg-primary px-4 py-1 text-on-primary">
            探す
          </button>
        </form>
        {draftQueryError !== null && <ResultMessage result={{ ok: false, message: draftQueryError }} />}
        <ResultMessage result={draftResult} />
        {drafts.kind === "loading" && <p className="text-sm text-text-muted">下書きを読み込み中…</p>}
        {drafts.kind === "failed" && <ResultMessage result={{ ok: false, message: drafts.message }} />}
        {drafts.kind === "loaded" && drafts.drafts.length === 0 && (
          <p className="text-sm">
            {drafts.q === "" ? "保存した下書きはありません" : `題に「${drafts.q}」を含む下書きはありません`}
          </p>
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
        <div className="space-y-4">
          <p className="text-sm">
            送ったお知らせを、送った日付（日本時間）で絞って、送った時刻の新しい順に見られます。期間は {MAX_PERIOD_DAYS}{" "}
            日まで選べます。スポーツを選ぶと、そのスポーツの生徒に向けて送ったものだけを出します（生徒全員に送ったものは「すべて」で見られます）。
          </p>
          <form className="flex flex-wrap items-end gap-4" onSubmit={(event) => void submitPeriod(event)}>
            <label className="block">
              開始日
              <input
                type="date"
                className="mt-1 block rounded-md border px-2 py-1"
                value={draftPeriod.from}
                onChange={(event) => setDraftPeriod({ ...draftPeriod, from: event.target.value })}
              />
            </label>
            <label className="block">
              終了日
              <input
                type="date"
                className="mt-1 block rounded-md border px-2 py-1"
                value={draftPeriod.to}
                onChange={(event) => setDraftPeriod({ ...draftPeriod, to: event.target.value })}
              />
            </label>
            <button type="submit" className="rounded-pill bg-primary px-4 py-1 text-on-primary">
              この期間で見る
            </button>
          </form>
          {periodDraftError !== null && <ResultMessage result={{ ok: false, message: periodDraftError }} />}
          <div className="flex flex-wrap items-end gap-4">
            <form className="flex items-end gap-2" onSubmit={(event) => void submitQuery(event)}>
              <label className="block">
                題で探す
                <input
                  type="search"
                  className="mt-1 block rounded-md border px-2 py-1"
                  value={queryDraft}
                  onChange={(event) => setQueryDraft(event.target.value)}
                />
              </label>
              <button type="submit" className="rounded-pill bg-primary px-4 py-1 text-on-primary">
                探す
              </button>
            </form>
            <label className="block">
              スポーツ
              <select
                className="mt-1 block rounded-md border px-2 py-1"
                value={sport}
                onChange={(event) => void changeSport(event.target.value as Sport | "")}
              >
                <option value="">すべて</option>
                {SPORTS.map((value) => (
                  <option key={value} value={value}>
                    {SPORT_LABELS[value]}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {queryError !== null && <ResultMessage result={{ ok: false, message: queryError }} />}
          <ResultMessage result={result} />
          {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
          {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
          {state.kind === "loaded" && (
            <div className="space-y-2">
              <p className="text-sm">
                {formatDay(state.period.from)} 〜 {formatDay(state.period.to)}
                {state.sport !== "" && `・${SPORT_LABELS[state.sport]}の生徒向け`}
                {state.q !== "" && `・題に「${state.q}」を含む`}・{state.announcements.length} 件
              </p>
              {state.announcements.length === 0 && <p className="text-sm">{emptyMessageOf(state)}</p>}
              {state.announcements.length > 0 && (
                <AnnouncementList announcements={state.announcements} onWithdraw={withdraw} />
              )}
            </div>
          )}
        </div>
      </Section>
    </>
  );
}
