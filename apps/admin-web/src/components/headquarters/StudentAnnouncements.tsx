"use client";

import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { formatDateTime } from "@/components/labels";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; announcements: Schemas["StudentAnnouncement"][] }
  | { kind: "failed"; message: string };

/**
 * 本部の「生徒の利用の様子」の「受け取ったお知らせ」。開いたときに生徒 1 人が受け取った本部からのお知らせ
 * （listStudentAnnouncements）を一度だけ読み、返った順（契約どおり送った新しい順に最大 50 件）のまま、題・送った日時・
 * 既読か・取り下げ済みかを出す。契約に読んだ時刻はないので既読は「既読」「未読」だけ。取り下げたお知らせも契約どおり
 * 一覧に残し、取り下げ済みと添える。0 件なら「受け取ったお知らせはありません」、失敗したときはこの欄の中にその文言を出す
 * （画面のほかの欄は別に読むので出し続ける）。照会だけで、既読にはしない。
 */
export function StudentAnnouncements({
  api,
  studentId,
}: {
  api: Pick<HeadquartersApi, "listStudentAnnouncements">;
  studentId: string;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let active = true;
    void api.listStudentAnnouncements(studentId).then((response) => {
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
  }, [api, studentId]);

  if (state.kind === "loading") return <p className="text-sm text-gray-500">読み込み中…</p>;
  if (state.kind === "failed") {
    return (
      <ResultMessage result={{ ok: false, message: `受け取ったお知らせを読めませんでした。${state.message}` }} />
    );
  }
  if (state.announcements.length === 0) return <p className="text-sm">受け取ったお知らせはありません</p>;

  return (
    <>
      <p className="mb-2 text-sm text-gray-600">
        送った新しい順に、最大 50 件まで出します。取り下げたお知らせは生徒のアプリの一覧には出ていません。
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
