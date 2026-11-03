"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Result } from "@/api/call";
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

type CoachesState =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Schemas["Coach"][] }
  | { kind: "failed"; message: string };

/**
 * 絞り込みの条件。`notBookedSince` は日付の入力欄の値（YYYY-MM-DD。空なら絞らない）。
 * `coachId` は予約したことのあるコーチの ID（空なら絞らない）。
 */
type Filter = { sport: Sport | ""; notBookedSince: string; coachId: string };

const NO_FILTER: Filter = { sport: "", notBookedSince: "", coachId: "" };

/** URL のコーチの ID がコーチの一覧にない（消えたコーチ・打ち間違い）ときの選択肢の名前。 */
const UNKNOWN_COACH = "（一覧にないコーチ）";

/**
 * サーバーの 422 は ErrorCode の汎用の文言しか出せないので、契約の 422 の条件を添えて直す場所を伝える。
 * スポーツと日付は入力欄が正しい値しか作らないので、ふつうは URL のコーチの ID が UUID でないときに来る。
 */
const VALIDATION_HINT = "コーチは一覧から選び直してください。";

function failureMessage(response: Extract<Result<unknown>, { ok: false }>): string {
  return response.code === "validation_failed" ? `${response.message}${VALIDATION_HINT}` : response.message;
}

const SPORTS = Object.keys(SPORT_LABELS) as Sport[];

/** 「直近 30 日の予約」の数え方（契約の `listStudents` の `recent_booking_count` の説明を短くしたもの）。 */
const RECENT_BOOKING_COUNT_NOTE =
  "直近 30 日の予約は、今日までの 30 日にその生徒が作った予約（体験予約を含む）のうち、キャンセルされていないものの数です。";

/** 契約種別とプランを 1 つの欄にする。受講権がなければ「未契約」。 */
function planLabel(student: Student): string {
  if (student.contract_type === null) return "未契約";
  const contract = CONTRACT_TYPE_LABELS[student.contract_type];
  return student.plan === null ? contract : `${contract}（${SUBSCRIPTION_PLAN_LABELS[student.plan]}）`;
}

/**
 * 本部の「生徒」。生徒の一覧（listStudents）を出す。絞り込み（スポーツ・この日以降に予約していない・予約したことのあるコーチ）は
 * API の `sport` / `not_booked_since` / `coach_id` に渡し、並びは契約の正本どおり返った順（登録の新しい順）のまま出す。
 * 各行には直近 30 日の予約の数（`recent_booking_count`。数え方は契約が正本で、画面は返った値を「n 件」で出すだけ。0 も「0 件」）
 * を出し、表の上に数え方を添える（並べ替え・絞り込みには使わない）。
 * コーチの選択肢はコーチの一覧（listCoaches）の返った順。`initialCoachId`（URL のクエリ）のコーチで絞って開き、
 * コーチを変えたら `onCoachChange` で知らせる（URL に残すのは呼ぶ側）。
 */
export function StudentDirectory({
  api,
  initialCoachId = "",
  onCoachChange,
}: {
  api: Pick<HeadquartersApi, "listStudents" | "listCoaches">;
  initialCoachId?: string;
  onCoachChange?: (coachId: string) => void;
}) {
  const [filter, setFilter] = useState<Filter>(() => ({ ...NO_FILTER, coachId: initialCoachId }));
  const [state, setState] = useState<State>({ kind: "loading" });
  const [coaches, setCoaches] = useState<CoachesState>({ kind: "loading" });
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchStudents = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listStudents({
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.notBookedSince === "" ? {} : { not_booked_since: next.notBookedSince }),
        ...(next.coachId === "" ? {} : { coach_id: next.coachId }),
      });
      return response.ok
        ? { kind: "loaded", students: response.data.students }
        : { kind: "failed", message: failureMessage(response) };
    },
    [api],
  );

  // 開いたときの読み込み（渡されたコーチだけで絞る）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchStudents({ ...NO_FILTER, coachId: initialCoachId }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchStudents, initialCoachId]);

  // コーチの選択肢。読めなくても生徒はほかの条件で引けるようにし、コーチの絞り込みだけを使えなくする。
  useEffect(() => {
    let active = true;
    void api.listCoaches().then((response) => {
      if (!active) return;
      setCoaches(
        response.ok
          ? { kind: "loaded", coaches: response.data.coaches }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api]);

  async function changeFilter(next: Filter) {
    setFilter(next);
    if (next.coachId !== filter.coachId) onCoachChange?.(next.coachId);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const loaded = await fetchStudents(next);
    if (request === latestRequest.current) setState(loaded);
  }

  const filtered = filter.sport !== "" || filter.notBookedSince !== "" || filter.coachId !== "";
  const unknownCoach =
    filter.coachId !== "" &&
    coaches.kind === "loaded" &&
    !coaches.coaches.some((coach) => coach.coach_id === filter.coachId);

  return (
    <div className="space-y-4">
      <p className="text-sm">
        登録している生徒を、登録の新しい順に見られます。コーチを選ぶと、そのコーチを予約したことのある生徒（キャンセルした予約を含む）に絞ります。表示名を押すと、その生徒の利用の様子を開きます。
      </p>
      <div className="flex flex-wrap items-end gap-4">
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
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
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.notBookedSince}
            onChange={(event) =>
              void changeFilter({
                ...filter,
                notBookedSince: event.target.value,
              })
            }
          />
        </label>
        <label className="block">
          コーチ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.coachId}
            disabled={coaches.kind !== "loaded"}
            onChange={(event) =>
              void changeFilter({
                ...filter,
                coachId: event.target.value,
              })
            }
          >
            <option value="">すべて</option>
            {coaches.kind === "loaded" &&
              coaches.coaches.map((coach) => (
                <option key={coach.coach_id} value={coach.coach_id}>
                  {coach.display_name}
                </option>
              ))}
            {unknownCoach && <option value={filter.coachId}>{UNKNOWN_COACH}</option>}
          </select>
        </label>
      </div>
      {coaches.kind === "failed" && (
        <ResultMessage
          result={{ ok: false, message: `コーチの一覧を読めませんでした。${coaches.message}` }}
        />
      )}
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.students.length === 0 && (
        <p className="text-sm">{filtered ? "条件に合う生徒はいません" : "まだ生徒はいません"}</p>
      )}
      {state.kind === "loaded" && state.students.length > 0 && (
        <>
          <p className="text-sm">{RECENT_BOOKING_COUNT_NOTE}</p>
          <table aria-label="生徒の一覧" className="w-full text-left text-sm">
            <thead>
              <tr>
                <th className="py-1">表示名</th>
                <th className="py-1">スポーツ</th>
                <th className="py-1">契約プラン</th>
                <th className="py-1">最後に予約した日</th>
                <th className="py-1">直近 30 日の予約</th>
                <th className="py-1">登録日</th>
              </tr>
            </thead>
            <tbody>
              {state.students.map((student) => (
                <tr key={student.student_id} className="border-t">
                  <td className="py-1">
                    <Link href={studentActivityPath(student.student_id)} className="text-primary underline">
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
                  <td className="py-1">{student.recent_booking_count} 件</td>
                  <td className="py-1">{formatDate(student.registered_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}
