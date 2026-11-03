"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime } from "@/components/labels";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; announcements: Schemas["StudentAnnouncement"][] }
  | { kind: "failed"; message: string };

/**
 * 本部の「生徒の利用の様子」の「受け取ったお知らせ」。開いたときに生徒 1 人が受け取った本部からのお知らせ
 * （listStudentAnnouncements）を読み、返った順（契約どおり送った新しい順に最大 50 件）のまま、題・送った日時・
 * 既読か・取り下げ済みかを出す。契約に読んだ時刻はないので既読は「既読」「未読」だけ。取り下げたお知らせも契約どおり
 * 一覧に残し、取り下げ済みと添える。「未読だけ」は API の `unread_only=true` に渡し、切り替えるたびに取り直す
 * （最大 50 件は API が絞ったあとに数える）。0 件なら「受け取ったお知らせはありません」（未読だけなら「未読のお知らせはありません」）、
 * 失敗したときはこの欄の中にその文言を出す（画面のほかの欄は別に読むので出し続ける）。照会だけで、既読にはしない。
 */
export function StudentAnnouncements({
  api,
  studentId,
}: {
  api: Pick<HeadquartersApi, "listStudentAnnouncements">;
  studentId: string;
}) {
  const [unreadOnly, setUnreadOnly] = useState(false);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 切り替えを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchAnnouncements = useCallback(
    async (onlyUnread: boolean): Promise<State> => {
      const response = await api.listStudentAnnouncements(studentId, onlyUnread ? { unread_only: true } : {});
      return response.ok
        ? { kind: "loaded", announcements: response.data.announcements }
        : { kind: "failed", message: response.message };
    },
    [api, studentId],
  );

  // 開いたときの読み込み（条件なし）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchAnnouncements(false).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchAnnouncements]);

  async function changeUnreadOnly(next: boolean) {
    setUnreadOnly(next);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchAnnouncements(next);
    if (request === latestRequest.current) setState(loaded);
  }

  return (
    <div className="space-y-2">
      <label className="flex items-center gap-1 text-sm">
        <input
          type="checkbox"
          checked={unreadOnly}
          onChange={(event) => void changeUnreadOnly(event.target.checked)}
        />
        未読だけ
      </label>
      <AnnouncementsBody state={state} unreadOnly={unreadOnly} />
    </div>
  );
}

function AnnouncementsBody({ state, unreadOnly }: { state: State; unreadOnly: boolean }) {
  if (state.kind === "loading") return <p className="text-sm text-text-muted">読み込み中…</p>;
  if (state.kind === "failed") {
    return (
      <ResultMessage result={{ ok: false, message: `受け取ったお知らせを読めませんでした。${state.message}` }} />
    );
  }
  if (state.announcements.length === 0) {
    return <p className="text-sm">{unreadOnly ? "未読のお知らせはありません" : "受け取ったお知らせはありません"}</p>;
  }

  return (
    <>
      <p className="text-sm text-text-muted">
        {unreadOnly ? "未読のものだけを、" : ""}送った新しい順に、最大 50 件まで出します。取り下げたお知らせは生徒のアプリの一覧には出ていません。
      </p>
      <table aria-label="受け取ったお知らせ" className="w-full text-left text-sm">
        <thead>
          <tr>
            <th className="py-1">題</th>
            <th className="py-1">送った日時</th>
            <th className="py-1">既読</th>
            <th className="py-1">取り下げ</th>
          </tr>
        </thead>
        <tbody>
          {state.announcements.map((announcement) => (
            <tr key={announcement.announcement_id} className="border-t">
              <td className="py-1">{announcement.title}</td>
              <td className="py-1">{formatDateTime(announcement.sent_at)}</td>
              <td className="py-1">{announcement.is_read ? "既読" : "未読"}</td>
              <td className="py-1">{announcement.is_withdrawn ? "取り下げ済み" : "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}
