"use client";

import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import {
  CONTRACT_TYPE_LABELS,
  SPORT_LABELS,
  SUBSCRIPTION_PLAN_LABELS,
  formatDate,
  formatDateTime,
} from "@/components/labels";

type Sport = Schemas["Sport"];
type Student = Schemas["Student"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; students: Student[] }
  | { kind: "failed"; message: string };

const SPORTS = Object.keys(SPORT_LABELS) as Sport[];

/** 契約種別とプランを 1 つの欄にする。受講権がなければ「未契約」。 */
function planLabel(student: Student): string {
  if (student.contract_type === null) return "未契約";
  const contract = CONTRACT_TYPE_LABELS[student.contract_type];
  return student.plan === null ? contract : `${contract}（${SUBSCRIPTION_PLAN_LABELS[student.plan]}）`;
}

/**
 * 本部の「生徒」。生徒の一覧（listStudents）を一度だけ読み、スポーツの絞り込みは画面の中でする
 * （契約に絞り込みがない）。並びは契約の正本どおり返った順（登録の新しい順）のまま出す。
 */
export function StudentDirectory({ api }: { api: Pick<HeadquartersApi, "listStudents"> }) {
  const [sport, setSport] = useState<Sport | "">("");
  const [state, setState] = useState<State>({ kind: "loading" });

  useEffect(() => {
    let active = true;
    void api.listStudents().then((response) => {
      if (!active) return;
      setState(
        response.ok
          ? { kind: "loaded", students: response.data.students }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api]);

  const students =
    state.kind === "loaded"
      ? state.students.filter((student) => sport === "" || student.sport === sport)
      : [];

  return (
    <div className="space-y-4">
      <p className="text-sm">登録している生徒を、登録の新しい順に見られます。</p>
      <label className="block">
        スポーツ
        <select
          className="mt-1 block rounded-md border px-2 py-1"
          value={sport}
          onChange={(event) => setSport(event.target.value as Sport | "")}
        >
          <option value="">すべて</option>
          {SPORTS.map((value) => (
            <option key={value} value={value}>
              {SPORT_LABELS[value]}
            </option>
          ))}
        </select>
      </label>
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.students.length === 0 && (
        <p className="text-sm">まだ生徒はいません</p>
      )}
      {state.kind === "loaded" && state.students.length > 0 && students.length === 0 && (
        <p className="text-sm">このスポーツの生徒はいません</p>
      )}
      {students.length > 0 && (
        <table aria-label="生徒の一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">表示名</th>
              <th className="py-1">スポーツ</th>
              <th className="py-1">契約プラン</th>
              <th className="py-1">最後の予約</th>
              <th className="py-1">登録日</th>
            </tr>
          </thead>
          <tbody>
            {students.map((student) => (
              <tr key={student.student_id} className="border-t">
                <td className="py-1">{student.display_name}</td>
                <td className="py-1">{SPORT_LABELS[student.sport]}</td>
                <td className="py-1">{planLabel(student)}</td>
                <td className="py-1">
                  {student.last_booked_at === null ? "—" : formatDateTime(student.last_booked_at)}
                </td>
                <td className="py-1">{formatDate(student.registered_at)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
