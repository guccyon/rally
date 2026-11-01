import type { Schemas } from "@/api/client";
import { announcementTargetLabel, formatDateTime } from "@/components/labels";

import { AnnouncementWithdrawAction } from "./AnnouncementWithdrawAction";

type Announcement = Schemas["Announcement"];

/**
 * 送ったお知らせの一覧。並びは渡された順（API の順＝送った時刻の新しい順）のまま、題・対象・受け取った生徒の数・
 * 送った日時・本文（改行はそのまま）を出す。取り下げていない行には「取り下げる」を、取り下げた行には
 * 「取り下げ済み（日時）」を出す（操作は出さない）。0 件の表示は呼び出し側。
 */
export function AnnouncementList({
  announcements,
  onWithdraw,
}: {
  announcements: Announcement[];
  onWithdraw: (announcement: Announcement) => Promise<void>;
}) {
  return (
    <ul aria-label="送ったお知らせの一覧" className="space-y-3">
      {announcements.map((announcement) => (
        <li key={announcement.announcement_id} className="rounded border p-3">
          <p className="font-semibold">{announcement.title}</p>
          <p className="text-xs text-gray-600">
            {announcementTargetLabel(announcement.sport)}・{announcement.recipient_count} 人に送信・
            {formatDateTime(announcement.sent_at)}
          </p>
          <p className="mt-2 whitespace-pre-wrap text-sm">{announcement.body}</p>
          <div className="mt-2 text-sm">
            {announcement.withdrawal === null ? (
              <AnnouncementWithdrawAction
                announcement={announcement}
                onWithdraw={() => onWithdraw(announcement)}
              />
            ) : (
              <span className="text-gray-600">
                取り下げ済み（{formatDateTime(announcement.withdrawal.withdrawn_at)}）
              </span>
            )}
          </div>
        </li>
      ))}
    </ul>
  );
}
