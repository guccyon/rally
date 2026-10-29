"use client";

import { useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

import { MessageThreadView } from "./MessageThreadView";

type MessageThread = Schemas["MessageThread"];

type ChatApi = Pick<
  CoachApi,
  | "listMessageThreads"
  | "listMyCoachBookings"
  | "listMessages"
  | "sendMessage"
  | "markMessageThreadRead"
  | "listMyCoachVideos"
  | "returnVideoReview"
>;

type ListState =
  | { kind: "loading" }
  | { kind: "loaded"; threads: MessageThread[]; names: Map<string, string> }
  | { kind: "failed"; message: string };

const UNKNOWN_NAME = "（名前を表示できません）";

/**
 * スレッドの生徒の表示名。スレッドは生徒の ID しか持たないので、コーチの自分の予約の一覧（listMyCoachBookings）の
 * 生徒の表示名から引く（スレッドは予約が確定したときに開くので、当事者の生徒は予約の一覧にいる）。
 */
async function fetchStudentNames(api: Pick<CoachApi, "listMyCoachBookings">) {
  const names = new Map<string, string>();
  const response = await api.listMyCoachBookings();
  if (!response.ok) return names;
  for (const item of response.data.bookings) {
    if (item.student_display_name) names.set(item.booking.student_id, item.student_display_name);
  }
  return names;
}

/** スレッドの一覧に出す最後のメッセージの短い文。添削の依頼と返却は種類から言い換え、知らない種類は `body` のまま。 */
function lastMessagePreview(message: Schemas["Message"]) {
  switch (message.kind) {
    case "video_review_request":
      return "動画の添削の依頼が届きました";
    case "video_review_returned":
      return "添削を返しました";
    default:
      return message.body;
  }
}

/**
 * コーチのチャット。開いたときに自分が当事者のスレッドの一覧（listMessageThreads。API の順＝最後の出来事の新しい順）を、
 * 生徒の名前と未読の数と最後のメッセージとともに出し、選んだスレッドのメッセージと送信の欄（MessageThreadView）を開く。
 * `initialThreadId`（対応待ちから移ったときのスレッド）があれば、そのスレッドを選んだ状態で開く。
 */
export function CoachChat({ api, initialThreadId }: { api: ChatApi; initialThreadId?: string }) {
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [selectedId, setSelectedId] = useState<string | null>(initialThreadId ?? null);

  const fetchThreads = useCallback(async (): Promise<ListState> => {
    const [threads, names] = await Promise.all([api.listMessageThreads(), fetchStudentNames(api)]);
    if (!threads.ok) return { kind: "failed", message: threads.message };
    return { kind: "loaded", threads: threads.data.threads, names };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchThreads().then((next) => {
      if (active) setList(next);
    });
    return () => {
      active = false;
    };
  }, [fetchThreads]);

  const reloadThreads = useCallback(async () => {
    const next = await fetchThreads();
    // 取り直しに失敗したときは、今の一覧を残す（開いているスレッドを閉じない）。
    if (next.kind === "loaded") setList(next);
  }, [fetchThreads]);

  // 既読にしたスレッド（unread_count が 0）を一覧に反映する。
  const onRead = useCallback((thread: MessageThread) => {
    setList((current) =>
      current.kind === "loaded"
        ? {
            ...current,
            threads: current.threads.map((item) =>
              item.thread_id === thread.thread_id ? { ...item, unread_count: thread.unread_count } : item,
            ),
          }
        : current,
    );
  }, []);

  if (list.kind === "loading") return <p className="text-sm text-gray-500">読み込み中…</p>;
  if (list.kind === "failed") return <ResultMessage result={{ ok: false, message: list.message }} />;
  if (list.threads.length === 0) {
    return (
      <p className="text-sm">
        まだチャットはありません。生徒の予約が確定すると、その生徒とのチャットがここに出ます。
      </p>
    );
  }

  const nameOf = (thread: MessageThread) => list.names.get(thread.student_id) ?? UNKNOWN_NAME;
  const selected = list.threads.find((thread) => thread.thread_id === selectedId) ?? null;

  return (
    <div className="space-y-4">
      <p className="text-sm">
        担当している生徒とのチャットです。生徒を選ぶと、やりとりを読んで返信できます。動画の添削の依頼もここから返せます。
      </p>
      <ul aria-label="生徒とのチャット" className="divide-y rounded border">
        {list.threads.map((thread) => {
          const last = thread.last_message;
          const isSelected = thread.thread_id === selectedId;
          return (
            <li key={thread.thread_id}>
              <button
                type="button"
                aria-pressed={isSelected}
                onClick={() => setSelectedId(thread.thread_id)}
                className={`block w-full px-3 py-2 text-left text-sm ${isSelected ? "bg-blue-50" : ""}`}
              >
                <span className="font-semibold">{nameOf(thread)}</span>
                {thread.unread_count > 0 && (
                  <span className="ml-2 rounded bg-red-600 px-2 text-xs text-white">
                    未読 {thread.unread_count} 件
                  </span>
                )}
                <span className="block truncate text-gray-600">
                  {last ? lastMessagePreview(last) : "まだメッセージはありません"}
                </span>
                {last && (
                  <span className="block text-xs text-gray-500">{formatDateTime(last.sent_at)}</span>
                )}
              </button>
            </li>
          );
        })}
      </ul>
      {selected && (
        <MessageThreadView
          key={selected.thread_id}
          api={api}
          threadId={selected.thread_id}
          studentName={nameOf(selected)}
          onRead={onRead}
          onChanged={reloadThreads}
        />
      )}
    </div>
  );
}
