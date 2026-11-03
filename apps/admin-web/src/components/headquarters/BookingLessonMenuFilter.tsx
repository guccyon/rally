"use client";

import { useEffect, useState } from "react";

import type { HeadquartersApi } from "@/api/headquarters";

type State = { kind: "loading" } | { kind: "loaded"; name: string } | { kind: "unknown" };

const NO_NAME = "（名前を表示できません）";

/**
 * 予約の一覧をレッスンメニューで絞っているときに、一覧の上に出す「どのレッスンメニューで絞っているか」。本部が 1 件を引く API は
 * ないので、レッスンメニューの一覧（listLessonMenus。絞り込みなし）を一度だけ読み、`lessonMenuId` の行の名前を出す（その期間に予約が
 * なくても名前を出せるように、予約の一覧の応答からは取らない）。一覧にない・読めないときは名前の代わりに「名前を表示できません」を出し、
 * 予約の一覧の読み込みは止めない（生徒の絞り込み BookingStudentFilter に揃える）。
 */
export function BookingLessonMenuFilter({
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
      const name = response.ok
        ? response.data.lesson_menus.find((menu) => menu.lesson_menu_id === lessonMenuId)?.name
        : undefined;
      setState(name === undefined ? { kind: "unknown" } : { kind: "loaded", name });
    });
    return () => {
      active = false;
    };
  }, [api, lessonMenuId]);

  return (
    <p className="mb-4 text-sm">
      レッスンメニューで絞っています:{" "}
      {state.kind === "loading" ? (
        <span className="text-text-muted">読み込み中…</span>
      ) : (
        <span className="font-semibold">{state.kind === "loaded" ? state.name : NO_NAME}</span>
      )}
    </p>
  );
}
