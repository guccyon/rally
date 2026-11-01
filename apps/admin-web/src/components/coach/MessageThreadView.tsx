"use client";

import { type FormEvent, type ReactNode, useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime, knownStamp, STAMPS } from "@/components/labels";

import { VideoReviewCommentForm } from "./VideoReviewCommentForm";

// 契約の SendMessageRequest.body の maxLength。
const BODY_MAX_LENGTH = 2000;

type Message = Schemas["Message"];
type CoachVideo = Schemas["CoachVideo"];
type StampId = Schemas["StampId"];

// 選ぶ欄に並べるスタンプ（STAMPS の順）。
const STAMP_IDS = Object.keys(STAMPS) as StampId[];

type ThreadState =
  | { kind: "loading" }
  | {
      kind: "loaded";
      messages: Message[];
      // 添削の状態を引くための、コーチに届いた動画（listMyCoachVideos）。読めなかったときは null。
      videos: Map<string, CoachVideo> | null;
    }
  | { kind: "failed"; message: string };

/** 添削の依頼が、このスレッドで何回目か（送った順）。返却のカードもこの番号でどの依頼への返却かを示す。 */
function requestNumbers(messages: Message[]) {
  const numbers = new Map<string, number>();
  for (const message of messages) {
    const videoId = message.kind === "video_review_request" ? message.video_id : undefined;
    if (videoId && !numbers.has(videoId)) numbers.set(videoId, numbers.size + 1);
  }
  return numbers;
}

/** 添削の依頼・返却のカードの枠。本文のメッセージと見分けられるよう、枠線と見出しを付ける。 */
function ReviewCard({
  title,
  sentAt,
  children,
}: {
  title: string;
  sentAt: string;
  children: ReactNode;
}) {
  return (
    <article
      aria-label={title}
      className="space-y-2 rounded border-2 border-primary-soft bg-primary-faint p-3 text-sm"
    >
      <h4 className="font-semibold">{title}</h4>
      {children}
      <p className="text-xs text-muted">{formatDateTime(sentAt)}</p>
    </article>
  );
}

/**
 * 1 つのスレッドのメッセージ（listMessages。送った順）と送信の欄（sendMessage）。開いたら既読にする（markMessageThreadRead）。
 * 本文のメッセージは送り手と一緒に、スタンプは絵柄を大きめに吹き出しなしで、生徒の添削の依頼とコーチの添削の返却はカードで
 * 出す。依頼のカードは、まだ返していない動画（listMyCoachVideos の添削待ち）なら「添削する」で「動画の添削」と同じ欄
 * （VideoReviewCommentForm）を開いて返す。送信の欄の「スタンプ」から 8 種類を選んで、本文の代わりに送れる。
 * 知らない種類のメッセージと知らないスタンプは `body`（短い説明）を本文のメッセージとして出す。
 */
export function MessageThreadView({
  api,
  threadId,
  studentName,
  onRead,
  onChanged,
}: {
  api: Pick<
    CoachApi,
    "listMessages" | "sendMessage" | "markMessageThreadRead" | "listMyCoachVideos" | "returnVideoReview"
  >;
  threadId: string;
  studentName: string;
  onRead: (thread: Schemas["MessageThread"]) => void;
  onChanged: () => void | Promise<void>;
}) {
  const [state, setState] = useState<ThreadState>({ kind: "loading" });
  const [reviewingVideoId, setReviewingVideoId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [pickingStamp, setPickingStamp] = useState(false);

  // メッセージと動画を読み、読めたら既読にする。
  const fetchThread = useCallback(async (): Promise<ThreadState> => {
    const [messages, videos] = await Promise.all([
      api.listMessages(threadId),
      api.listMyCoachVideos(),
    ]);
    if (!messages.ok) return { kind: "failed", message: messages.message };
    const read = await api.markMessageThreadRead(threadId);
    if (read.ok) onRead(read.data);
    return {
      kind: "loaded",
      messages: messages.data.messages,
      videos: videos.ok
        ? new Map(videos.data.videos.map((video) => [video.video_id, video]))
        : null,
    };
  }, [api, threadId, onRead]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchThread().then((next) => {
      if (active) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchThread]);

  async function onReload() {
    setBusy(true);
    setState(await fetchThread());
    setBusy(false);
  }

  // 本文かスタンプを送り、送れたらスレッドに足す。送れたかを返す。
  async function send(request: Schemas["SendMessageRequest"]) {
    setBusy(true);
    const response = await api.sendMessage(threadId, request);
    if (response.ok) {
      setResult(null);
      setState((current) =>
        current.kind === "loaded"
          ? { ...current, messages: [...current.messages, response.data] }
          : current,
      );
      await onChanged();
    } else {
      setResult(response);
    }
    setBusy(false);
    return response.ok;
  }

  async function onSend(event: FormEvent) {
    event.preventDefault();
    const body = draft.trim();
    if (body === "") {
      setResult({ ok: false, message: "メッセージを入力してください。" });
      return;
    }
    if (await send({ body })) setDraft("");
  }

  async function onSendStamp(stampId: StampId) {
    if (await send({ stamp_id: stampId })) setPickingStamp(false);
  }

  async function onReturned(review: Schemas["VideoReview"]) {
    setReviewingVideoId(null);
    setResult({
      ok: true,
      message: review.returned_at
        ? `${studentName}に添削を返却しました（${formatDateTime(review.returned_at)}）。`
        : `${studentName}に添削を返却しました。`,
    });
    // 返却のメッセージがスレッドに足され、動画は返却済みになるので、両方を取り直す。
    setState(await fetchThread());
    await onChanged();
  }

  function renderRequest(message: Message, number: number | undefined) {
    const title = number === undefined ? "添削の依頼" : `添削の依頼（${number} 回目）`;
    const video =
      state.kind === "loaded" && message.video_id ? state.videos?.get(message.video_id) : undefined;
    return (
      <ReviewCard title={title} sentAt={message.sent_at}>
        <p>{studentName}が動画を送って、添削を依頼しました。</p>
        {!video ? (
          <p className="text-secondary">
            添削の状態を確かめられませんでした。「動画の添削」の一覧から確かめてください。
          </p>
        ) : video.review_status === "returned" ? (
          <p className="text-secondary">
            返却済み{video.returned_at ? `（${formatDateTime(video.returned_at)}）` : ""}
          </p>
        ) : reviewingVideoId === video.video_id ? (
          <VideoReviewCommentForm
            api={api}
            video={video}
            studentName={studentName}
            onReturned={onReturned}
            onCancel={() => setReviewingVideoId(null)}
          />
        ) : (
          <div className="flex items-center gap-2">
            <span className="text-secondary">まだ添削を返していません。</span>
            <button
              type="button"
              disabled={busy}
              onClick={() => {
                setReviewingVideoId(video.video_id);
                setResult(null);
              }}
              className="rounded bg-primary px-3 py-1 text-inverse disabled:opacity-50"
            >
              添削する
            </button>
          </div>
        )}
      </ReviewCard>
    );
  }

  function renderReturned(message: Message, number: number | undefined) {
    const title = number === undefined ? "添削の返却" : `添削の返却（${number} 回目の依頼）`;
    return (
      <ReviewCard title={title} sentAt={message.sent_at}>
        <p>あなたが動画の添削を返しました。</p>
      </ReviewCard>
    );
  }

  function renderText(message: Message) {
    const mine = message.sender_role === "coach";
    const sender = mine ? "あなた" : message.sender_role === "student" ? studentName : "お知らせ";
    return (
      <div className={`rounded p-2 text-sm ${mine ? "ml-8 bg-success-faint" : "mr-8 bg-muted"}`}>
        <p className="text-xs font-semibold">{sender}</p>
        <p className="whitespace-pre-wrap">{message.body}</p>
        <p className="text-xs text-muted">
          {formatDateTime(message.sent_at)}
          {mine && message.read_at ? "・既読" : ""}
        </p>
      </div>
    );
  }

  function renderStamp(message: Message, stamp: { emoji: string; label: string }) {
    const mine = message.sender_role === "coach";
    const sender = mine ? "あなた" : message.sender_role === "student" ? studentName : "お知らせ";
    return (
      <div className={`flex flex-col text-sm ${mine ? "ml-8 items-end" : "mr-8 items-start"}`}>
        <p className="text-xs font-semibold">{sender}</p>
        <div
          role="img"
          aria-label={`スタンプ: ${stamp.label}`}
          className="flex flex-col items-center"
        >
          <span className="text-5xl leading-tight">{stamp.emoji}</span>
          <span>{stamp.label}</span>
        </div>
        <p className="text-xs text-muted">
          {formatDateTime(message.sent_at)}
          {mine && message.read_at ? "・既読" : ""}
        </p>
      </div>
    );
  }

  function renderMessage(message: Message, numbers: Map<string, number>) {
    const number = message.video_id ? numbers.get(message.video_id) : undefined;
    const stamp = message.kind === "stamp" ? knownStamp(message.stamp_id) : undefined;
    if (stamp) return renderStamp(message, stamp);
    switch (message.kind) {
      case "video_review_request":
        return renderRequest(message, number);
      case "video_review_returned":
        return renderReturned(message, number);
      default:
        // 本文と、知らない種類・知らないスタンプ（`body` が短い説明）。
        return renderText(message);
    }
  }

  const numbers =
    state.kind === "loaded" ? requestNumbers(state.messages) : new Map<string, number>();

  return (
    <section aria-label={`${studentName}とのチャット`} className="space-y-3 rounded border p-3">
      <div className="flex items-center justify-between">
        <h3 className="font-semibold">{studentName}とのチャット</h3>
        <button
          type="button"
          disabled={busy || state.kind === "loading"}
          onClick={() => void onReload()}
          className="rounded border px-3 py-1 text-sm disabled:opacity-50"
        >
          新しいメッセージを確かめる
        </button>
      </div>
      {state.kind === "loading" && <p className="text-sm text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" &&
        (state.messages.length === 0 ? (
          <p className="text-sm">まだメッセージはありません。</p>
        ) : (
          <ol aria-label="メッセージ" className="space-y-2">
            {state.messages.map((message) => (
              <li key={message.message_id}>{renderMessage(message, numbers)}</li>
            ))}
          </ol>
        ))}
      {state.kind === "loaded" && (
        <form onSubmit={onSend} aria-label="メッセージを送る" className="space-y-2">
          <label className="block text-sm">
            送るメッセージ
            <textarea
              className="mt-1 block w-full rounded border px-2 py-1"
              rows={3}
              maxLength={BODY_MAX_LENGTH}
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
            />
          </label>
          <div className="flex gap-2">
            <button
              type="submit"
              disabled={busy}
              className="rounded bg-primary px-4 py-1 text-inverse disabled:opacity-50"
            >
              送る
            </button>
            <button
              type="button"
              disabled={busy}
              aria-expanded={pickingStamp}
              onClick={() => setPickingStamp((current) => !current)}
              className="rounded border px-3 py-1 text-sm disabled:opacity-50"
            >
              スタンプ
            </button>
          </div>
          {pickingStamp && (
            <fieldset className="space-y-2 rounded border p-2">
              <legend className="px-1 text-sm">送るスタンプを選んでください</legend>
              <div className="flex flex-wrap gap-2">
                {STAMP_IDS.map((stampId) => (
                  <button
                    key={stampId}
                    type="button"
                    disabled={busy}
                    aria-label={`スタンプ「${STAMPS[stampId].label}」を送る`}
                    onClick={() => void onSendStamp(stampId)}
                    className="flex w-24 flex-col items-center rounded border py-1 text-sm disabled:opacity-50"
                  >
                    <span aria-hidden="true" className="text-3xl">
                      {STAMPS[stampId].emoji}
                    </span>
                    <span aria-hidden="true">{STAMPS[stampId].label}</span>
                  </button>
                ))}
              </div>
            </fieldset>
          )}
        </form>
      )}
      <ResultMessage result={result} />
    </section>
  );
}
