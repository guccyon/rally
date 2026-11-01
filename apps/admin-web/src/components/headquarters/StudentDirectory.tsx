"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import {
  CONTRACT_TYPE_LABELS,
  SPORT_LABELS,
  SUBSCRIPTION_PLAN_LABELS,
  formatDate,
} from "@/components/labels";

import { studentActivityPath } from "./StudentActivityHome";

type Sport = Schemas["Sport"];
type Student = Schemas["Student"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; students: Student[] }
  | { kind: "failed"; message: string };

/** 絞り込みの条件。`notBookedSince` は日付の入力欄の値（YYYY-MM-DD。空なら絞らない）。 */
type Filter = { sport: Sport | ""; notBookedSince: string };

const NO_FILTER: Filter = { sport: "", notBookedSince: "" };

const SPORTS = Object.keys(SPORT_LABELS) as Sport[];

/** 契約種別とプランを 1 つの欄にする。受講権がなければ「未契約」。 */
function planLabel(student: Student): string {
  if (student.contract_type === null) return "未契約";
  const contract = CONTRACT_TYPE_LABELS[student.contract_type];
  return student.plan === null ? contract : `${contract}（${SUBSCRIPTION_PLAN_LABELS[student.plan]}）`;
}

/**
 * 本部の「生徒」。生徒の一覧（listStudents）を出す。絞り込み（スポーツ・この日以降に予約していない）は API の
 * `sport` / `not_booked_since` に渡し、並びは契約の正本どおり返った順（登録の新しい順）のまま出す。
 */
export function StudentDirectory({ api }: { api: Pick<HeadquartersApi, "listStudents"> }) {
  const [filter, setFilter] = useState<Filter>(NO_FILTER);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchStudents = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listStudents({
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.notBookedSince === "" ? {} : { not_booked_since: next.notBookedSince }),
      });
      return response.ok
        ? { kind: "loaded", students: response.data.students }
        : { kind: "failed", message: response.message };
    },
    [api],
  );

  // 開いたときの読み込み（条件なし）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchStudents(NO_FILTER).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchStudents]);

  async function changeFilter(next: Filter) {
    setFilter(next);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchStudents(next);
    if (request === latestRequest.current) setState(loaded);
  }

  const filtered = filter.sport !== "" || filter.notBookedSince !== "";

  return (
    <div className="space-y-4">
      <p className="text-sm">
        登録している生徒を、登録の新しい順に見られます。表示名を押すと、その生徒の利用の様子を開きます。
      </p>
      <div className="flex flex-wrap items-end gap-4">
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded border px-2 py-1"
            value={filter.sport}
            onChange={(event) =>
              void changeFilter({
                ...filter,
                sport: event.target.value as Sport | "",
              })
            }
          >
            <option value="">すべて</option>
            {SPORTS.map((value) => (
              <option key={value} value={value}>
                {SPORT_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
        <label className="block">
          この日以降に予約していない
          <input
            type="date"
            className="mt-1 block rounded border px-2 py-1"
            value={filter.notBookedSince}
            onChange={(event) =>
              void changeFilter({
                ...filter,
                notBookedSince: event.target.value,
              })
            }
          />
        </label>
      </div>
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.students.length === 0 && (
        <p className="text-sm">{filtered ? "条件に合う生徒はいません" : "まだ生徒はいません"}</p>
      )}
      {state.kind === "loaded" && state.students.length > 0 && (
        <table aria-label="生徒の一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">表示名</th>
              <th className="py-1">スポーツ</th>
              <th className="py-1">契約プラン</th>
              <th className="py-1">最後に予約した日</th>
              <th className="py-1">登録日</th>
            </tr>
          </thead>
          <tbody>
            {state.students.map((student) => (
              <tr key={student.student_id} className="border-t">
                <td className="py-1">
                  <Link href={studentActivityPath(student.student_id)} className="text-blue-600 underline">
                    {student.display_name}
                  </Link>
                </td>
                <td className="py-1">{SPORT_LABELS[student.sport]}</td>
                <td className="py-1">{planLabel(student)}</td>
                <td className="py-1">
                  {student.last_booked_at === null
                    ? "まだ予約がありません"
                    : formatDate(student.last_booked_at)}
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
