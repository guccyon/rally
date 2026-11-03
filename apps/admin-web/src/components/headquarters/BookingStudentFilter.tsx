"use client";

import { useEffect, useState } from "react";

import type { HeadquartersApi } from "@/api/headquarters";

type State = { kind: "loading" } | { kind: "loaded"; name: string } | { kind: "unknown" };

const NO_NAME = "（名前を表示できません）";

/**
 * 予約の一覧・返した動画添削の記録を生徒で絞っているときに、一覧の上に出す「どの生徒で絞っているか」。生徒の表示名は
 * getStudentActivity で読む（その期間に予約・記録がなくても名前を出せるように、一覧の応答からは取らない）。読めない（404・ほかの失敗）ときは名前の
 * 代わりに「名前を表示できません」を出し、一覧の読み込みは止めない。
 */
export function BookingStudentFilter({
  api,
  studentId,
}: {
  api: Pick<HeadquartersApi, "getStudentActivity">;
  studentId: string;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let active = true;
    void api.getStudentActivity(studentId).then((response) => {
      if (!active) return;
      setState(response.ok ? { kind: "loaded", name: response.data.display_name } : { kind: "unknown" });
    });
    return () => {
      active = false;
    };
  }, [api, studentId]);

  return (
    <p className="mb-4 text-sm">
      生徒で絞っています:{" "}
      {state.kind === "loading" ? (
        <span className="text-text-muted">読み込み中…</span>
      ) : (
        <span className="font-semibold">{state.kind === "loaded" ? state.name : NO_NAME}</span>
      )}
    </p>
  );
}
