"use client";

import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import { formatDateTime } from "@/components/labels";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; changes: Schemas["LessonMenuPublicationChange"][] }
  | { kind: "not_found" }
  | { kind: "failed"; message: string };

const ACTION_LABELS: Record<Schemas["LessonMenuPublicationAction"], string> = {
  unpublished: "非公開にした",
  republished: "公開に戻した",
};

const NOT_FOUND = "このレッスンメニューは見つかりません。レッスンメニューの一覧から選び直してください。";

/** 公開に戻した記録（null）と、理由を書かずに非公開にした記録（空文字列）は「—」。 */
function reasonLabel(reason: string | null): string {
  return reason === null || reason === "" ? "—" : reason;
}

/**
 * 本部の「公開の記録」。開いたときにレッスンメニュー 1 件を本部が非公開にした・公開に戻した記録
 * （listLessonMenuPublicationChanges）を一度だけ読み、返った順（契約どおり新しい順）のまま、操作・日時・
 * 操作した本部のアカウント（契約にあるのは ID だけ）・理由を出す。0 件なら「本部が非公開にしたことはありません」。
 * `lessonMenuId` がない・API が 404 のときは「見つからない」として、ほかの失敗とは分けて出す。公開・非公開は変えない。
 */
export function LessonMenuPublicationHistory({
  api,
  lessonMenuId,
}: {
  api: Pick<HeadquartersApi, "listLessonMenuPublicationChanges">;
  lessonMenuId: string | undefined;
}) {
  const [state, setState] = useState<State>(lessonMenuId ? { kind: "loading" } : { kind: "not_found" });

  useEffect(() => {
    if (!lessonMenuId) return;
    let active = true;
    void api.listLessonMenuPublicationChanges(lessonMenuId).then((response) => {
      if (!active) return;
      if (response.ok) setState({ kind: "loaded", changes: response.data.changes });
      else if (response.code === "not_found") setState({ kind: "not_found" });
      else setState({ kind: "failed", message: response.message });
    });
    return () => {
      active = false;
    };
  }, [api, lessonMenuId]);

  if (state.kind === "loading") return <p className="text-sm text-text-muted">読み込み中…</p>;
  if (state.kind === "not_found") return <ResultMessage result={{ ok: false, message: NOT_FOUND }} />;
  if (state.kind === "failed") {
    return (
      <ResultMessage result={{ ok: false, message: `公開の記録を読めませんでした。${state.message}` }} />
    );
  }
  if (state.changes.length === 0) return <p className="text-sm">本部が非公開にしたことはありません</p>;

  return (
    <table aria-label="公開の記録" className="w-full text-left text-sm">
      <thead>
        <tr>
          <th className="py-1">操作</th>
          <th className="py-1">日時</th>
          <th className="py-1">操作した本部のアカウント</th>
          <th className="py-1">理由</th>
        </tr>
      </thead>
      <tbody>
        {state.changes.map((change, index) => (
          // 記録に ID はなく、同じ時刻もありうるので、返った順の位置で区別する（並べ替えはしない）。
          <tr key={index} className="border-t">
            <td className="py-1">{ACTION_LABELS[change.action]}</td>
            <td className="py-1">{formatDateTime(change.changed_at)}</td>
            <td className="py-1 font-mono text-xs">{change.changed_by}</td>
            <td className="py-1">{reasonLabel(change.reason)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
