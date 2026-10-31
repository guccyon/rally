"use client";

import { useEffect, useState } from "react";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

type Notification = Schemas["Notification"];

/** 未読のお知らせの数（見出しの「お知らせ」のバッジ）。 */
export function countUnreadNotifications(notifications: Notification[]): number {
  return notifications.filter((notification) => !notification.is_read).length;
}

/**
 * コーチのお知らせ（glossary の notification）の一覧。開いたときに一覧を読み（新しい順＝API の順のまま）、
 * 未読があればすべて既読にする（markMyNotificationsRead）。開いたときに未読だったものには「新着」と添える。
 * 既読にできなくても一覧は出し（失敗の文言は出さない）、未読のまま次に開いたときにもう一度既読にする。
 * お知らせは行き先の ID を持たないので、行を押すと種類に応じた画面（`destinations` にある種類だけ）へ移る。
 * 知らない種類の行は押せない。
 */
export function CoachNotifications({
  api,
  destinations,
  onLoaded,
  onOpen,
}: {
  api: Pick<CoachApi, "listMyNotifications" | "markMyNotificationsRead">;
  /** 種類 → 行を押したときに移る画面の名前 */
  destinations: Partial<Record<string, string>>;
  /** 一覧を読んだ（既読にした）あとの一覧。見出しの未読の数を直すのに使う */
  onLoaded: (notifications: Notification[]) => void;
  onOpen: (kind: Schemas["NotificationKind"]) => void;
}) {
  const [result, setResult] = useState<Result<Notification[]> | null>(null);
  const [newIds, setNewIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    let active = true;
    void (async () => {
      const listed = await api.listMyNotifications();
      if (!active) return;
      if (!listed.ok) {
        setResult(listed);
        return;
      }
      const unread = listed.data.notifications.filter((notification) => !notification.is_read);
      setNewIds(new Set(unread.map((notification) => notification.notification_id)));
      setResult({ ok: true, data: listed.data.notifications });
      if (unread.length === 0) {
        onLoaded(listed.data.notifications);
        return;
      }
      const read = await api.markMyNotificationsRead();
      if (!active) return;
      if (!read.ok) {
        onLoaded(listed.data.notifications);
        return;
      }
      // 読んだあとに届いたお知らせも、既読にした一覧に入れば新着として見分ける
      const listedIds = new Set(listed.data.notifications.map((notification) => notification.notification_id));
      setNewIds((prev) => {
        const next = new Set(prev);
        for (const notification of read.data.notifications) {
          if (!listedIds.has(notification.notification_id)) next.add(notification.notification_id);
        }
        return next;
      });
      setResult({ ok: true, data: read.data.notifications });
      onLoaded(read.data.notifications);
    })();
    return () => {
      active = false;
    };
    // 開いたときに 1 回だけ読む（onLoaded は親が描き直すたびに変わる）
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api]);

  if (!result) return <p className="text-sm text-gray-500">読み込み中…</p>;
  if (!result.ok) return <ResultMessage result={{ ok: false, message: result.message }} />;
  if (result.data.length === 0) {
    return (
      <p className="text-sm">
        まだお知らせはありません。新しい予約・動画の添削の依頼・生徒からのメッセージがここに届きます。
      </p>
    );
  }
  return (
    <ul aria-label="お知らせの一覧" className="divide-y rounded border text-sm">
      {result.data.map((notification) => {
        const destination = destinations[notification.kind];
        const content = (
          <>
            <span className="block">
              {newIds.has(notification.notification_id) && (
                <span className="mr-2 rounded bg-red-600 px-2 text-xs text-white">新着</span>
              )}
              {notification.body}
            </span>
            <span className="block text-gray-600">{formatDateTime(notification.created_at)}</span>
          </>
        );
        return (
          <li key={notification.notification_id}>
            {destination ? (
              <button
                type="button"
                onClick={() => onOpen(notification.kind)}
                className="flex w-full items-center justify-between gap-2 px-3 py-2 text-left hover:bg-gray-50"
              >
                <span>{content}</span>
                <span className="shrink-0 text-blue-700">「{destination}」へ</span>
              </button>
            ) : (
              <div className="px-3 py-2">{content}</div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
