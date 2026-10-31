"use client";

import type { ReactNode } from "react";

import type { Result } from "@/api/call";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

import type { PendingTasks } from "./pendingTasks";
import { UNKNOWN_NAME } from "./studentNames";
import { videoLessonMenuLabel } from "./videoLessonMenu";

const BUTTON = "rounded bg-blue-600 px-3 py-1 text-white";

/** 対応待ちの 1 種類（見出し・件数・一覧）。一覧の API が失敗したときはその文言だけを出す。 */
function TaskGroup<T>({
  title,
  note,
  part,
  empty,
  renderItem,
}: {
  title: string;
  note?: string;
  part: Result<T[]>;
  empty: string;
  renderItem: (item: T) => ReactNode;
}) {
  return (
    <div className="space-y-2">
      <h3 className="font-semibold">
        {title}
        {part.ok && `（${part.data.length} 件）`}
      </h3>
      {note && <p className="text-sm text-gray-600">{note}</p>}
      {!part.ok && <ResultMessage result={{ ok: false, message: part.message }} />}
      {part.ok && part.data.length === 0 && <p className="text-sm">{empty}</p>}
      {part.ok && part.data.length > 0 && (
        <ul aria-label={title} className="divide-y rounded border text-sm">
          {part.data.map(renderItem)}
        </ul>
      )}
    </div>
  );
}

/**
 * コーチの「対応待ち」。添削待ちの動画（送られた古い順）・生徒の確定を待っている予約（日時の早い順）・
 * 未読のあるチャット（最後の出来事の新しい順）を並べ、それぞれのボタンで該当の画面へ移る。
 * 添削待ちの動画には、送った時点のレッスンメニューがあれば「メニュー: 〇〇」も出す。
 */
export function CoachTasks({
  tasks,
  onOpenVideo,
  onOpenBookings,
  onOpenThread,
}: {
  tasks: PendingTasks | null;
  onOpenVideo: (videoId: string) => void;
  onOpenBookings: () => void;
  onOpenThread: (threadId: string) => void;
}) {
  if (!tasks) return <p className="text-sm text-gray-500">読み込み中…</p>;
  return (
    <div className="space-y-6">
      <p className="text-sm">
        添削を待っている動画、確定を待っている予約、まだ読んでいないメッセージのあるチャットです。ボタンから、それぞれの画面へ移れます。
      </p>
      <TaskGroup
        title="添削待ちの動画"
        part={tasks.videos}
        empty="添削待ちの動画はありません。"
        renderItem={(video) => (
          <li key={video.video_id} className="flex items-center justify-between gap-2 px-3 py-2">
            <span>
              <span className="font-semibold">{video.student_display_name ?? UNKNOWN_NAME}</span>
              <span className="block text-gray-600">
                {formatDateTime(video.submitted_at)} に送られました
              </span>
              {video.lesson_menu && (
                <span className="block text-gray-600">{videoLessonMenuLabel(video.lesson_menu)}</span>
              )}
            </span>
            <button type="button" onClick={() => onOpenVideo(video.video_id)} className={BUTTON}>
              添削する
            </button>
          </li>
        )}
      />
      <TaskGroup
        title="確定待ちの予約"
        note="生徒がまだ確定していない予約です。確定すると通話を始められます。"
        part={tasks.bookings}
        empty="確定待ちの予約はありません。"
        renderItem={(item) => (
          <li
            key={item.booking.booking_id}
            className="flex items-center justify-between gap-2 px-3 py-2"
          >
            <span>
              <span className="font-semibold">{item.student_display_name ?? UNKNOWN_NAME}</span>
              <span className="block text-gray-600">
                {formatDateTime(item.slot.starts_at)}〜{formatDateTime(item.slot.ends_at)}
              </span>
            </span>
            <button type="button" onClick={onOpenBookings} className={BUTTON}>
              予約を見る
            </button>
          </li>
        )}
      />
      <TaskGroup
        title="未読のあるチャット"
        part={tasks.threads}
        empty="まだ読んでいないメッセージはありません。"
        renderItem={(thread) => (
          <li key={thread.thread_id} className="flex items-center justify-between gap-2 px-3 py-2">
            <span>
              <span className="font-semibold">
                {tasks.names.get(thread.student_id) ?? UNKNOWN_NAME}
              </span>
              <span className="ml-2 rounded bg-red-600 px-2 text-xs text-white">
                未読 {thread.unread_count} 件
              </span>
            </span>
            <button type="button" onClick={() => onOpenThread(thread.thread_id)} className={BUTTON}>
              チャットを開く
            </button>
          </li>
        )}
      />
    </div>
  );
}
