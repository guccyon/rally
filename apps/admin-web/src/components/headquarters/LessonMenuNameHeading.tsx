"use client";

import { useEffect, useState } from "react";

import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";

type State =
  | { kind: "loading" }
  | { kind: "loaded"; name: string | undefined }
  | { kind: "failed"; message: string };

const NO_NAME = "（名前を表示できません）";

/**
 * 本部の「公開の記録」の上に出す、レッスンメニュー 1 件の名前の見出し。本部が 1 件を引く API はないので、
 * 開いたときにレッスンメニューの一覧（listLessonMenus。絞り込みなし）を一度だけ読み、`lessonMenuId` の行の名前を出す。
 * 一覧にないときは「（名前を表示できません）」、一覧を読めないときはそれに加えてその文言を出す（記録の表は出し続ける）。
 */
export function LessonMenuNameHeading({
  api,
  lessonMenuId,
}: {
  api: Pick<HeadquartersApi, "listLessonMenus">;
  lessonMenuId: string;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let active = true;
    void api.listLessonMenus({}).then((response) => {
      if (!active) return;
      setState(
        response.ok
          ? {
              kind: "loaded",
              name: response.data.lesson_menus.find((menu) => menu.lesson_menu_id === lessonMenuId)?.name,
            }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api, lessonMenuId]);

  // 読み込み中は記録の表の側が「読み込み中…」を出すので、見出しは出さない。
  if (state.kind === "loading") return null;
  return (
    <div className="mb-2 space-y-2">
      <h3 className="text-base font-semibold">{(state.kind === "loaded" && state.name) || NO_NAME}</h3>
      {state.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `レッスンメニューの名前を読めませんでした。${state.message}` }}
        />
      )}
    </div>
  );
}
