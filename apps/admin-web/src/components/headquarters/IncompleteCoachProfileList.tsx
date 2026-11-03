"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ui/ResultMessage";
import {
  COACH_ACCEPTANCE_STATUS_LABELS,
  COACH_PROFILE_FIELD_LABELS,
  SPORT_LABELS,
  formatDateTime,
} from "@/components/labels";

import { ACCEPTANCE_STATUSES, SPORTS } from "./CoachAcceptanceList";
import { coachOverviewPath } from "./CoachOverviewHome";

type IncompleteCoachProfile = Schemas["IncompleteCoachProfile"];
type Sport = Schemas["Sport"];
type AcceptanceStatus = Schemas["CoachAcceptanceStatus"];

/** 絞り込みの条件。空なら絞らない。 */
export type IncompleteCoachProfileFilter = { sport: Sport | ""; acceptanceStatus: AcceptanceStatus | "" };

const NO_FILTER: IncompleteCoachProfileFilter = { sport: "", acceptanceStatus: "" };

type State =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: IncompleteCoachProfile[] }
  | { kind: "failed"; message: string };

/** 最後に更新した日時。契約は記録がなければ null で、その間は登録した時刻を目安にする。 */
function updatedAtLabel(coach: IncompleteCoachProfile): string {
  return coach.updated_at === null
    ? `記録なし（登録: ${formatDateTime(coach.registered_at)}）`
    : formatDateTime(coach.updated_at);
}

/**
 * 本部の「自己紹介の足りないコーチ」。開いたときにプロフィールに空の項目があるコーチの一覧
 * （listIncompleteCoachProfiles）を読み、コーチ（そのコーチの様子へのリンク）・空の項目・最後に更新した日時を出す。
 * 「足りない」の判定と並べ方は契約の正本どおり API が済ませているので、返った順（登録の古い順）のまま出す。
 * 照会だけで、コーチへの連絡はしない。
 * 一覧の上の絞り込み（スポーツ・受付の状態）は API の `sport` / `acceptance_status` に渡して読み直す（組み合わせられる）。
 * 受付の状態の選択肢と言葉はコーチの受付の一覧と同じ。`initialFilter`（URL のクエリ）の条件で開き、
 * 変えたら `onFilterChange` で知らせる（URL に残すのは呼ぶ側）。
 */
export function IncompleteCoachProfileList({
  api,
  initialFilter = NO_FILTER,
  onFilterChange,
}: {
  api: Pick<HeadquartersApi, "listIncompleteCoachProfiles">;
  initialFilter?: IncompleteCoachProfileFilter;
  onFilterChange?: (filter: IncompleteCoachProfileFilter) => void;
}) {
  const [filter, setFilter] = useState<IncompleteCoachProfileFilter>(initialFilter);
  const [state, setState] = useState<State>({ kind: "loading" });
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchCoaches = useCallback(
    async (next: IncompleteCoachProfileFilter): Promise<State> => {
      const response = await api.listIncompleteCoachProfiles({
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.acceptanceStatus === "" ? {} : { acceptance_status: next.acceptanceStatus }),
      });
      return response.ok
        ? { kind: "loaded", coaches: response.data.coaches }
        : { kind: "failed", message: response.message };
    },
    [api],
  );

  // 開いたときの読み込み（渡された条件）。初期の状態が loading なので、ここでは結果だけを入れる。
  const { sport: initialSport, acceptanceStatus: initialAcceptanceStatus } = initialFilter;
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchCoaches({ sport: initialSport, acceptanceStatus: initialAcceptanceStatus }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchCoaches, initialSport, initialAcceptanceStatus]);

  async function changeFilter(next: IncompleteCoachProfileFilter) {
    setFilter(next);
    onFilterChange?.(next);
    setState({ kind: "loading" });
    const request = ++latestRequest.current;
    const loaded = await fetchCoaches(next);
    if (request === latestRequest.current) setState(loaded);
  }

  const filtered = filter.sport !== "" || filter.acceptanceStatus !== "";

  return (
    <div className="space-y-4">
      <p className="text-sm">
        自己紹介に空の項目があるコーチを、登録の古い順に見られます。書きかけのままのコーチを見つけて声をかけるための一覧です。
      </p>
      <div className="flex flex-wrap items-end gap-4">
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.sport}
            onChange={(event) => void changeFilter({ ...filter, sport: event.target.value as Sport | "" })}
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
          受付の状態
          <select
            className="mt-1 block rounded-md border px-2 py-1"
            value={filter.acceptanceStatus}
            onChange={(event) =>
              void changeFilter({ ...filter, acceptanceStatus: event.target.value as AcceptanceStatus | "" })
            }
          >
            <option value="">すべて</option>
            {ACCEPTANCE_STATUSES.map((value) => (
              <option key={value} value={value}>
                {COACH_ACCEPTANCE_STATUS_LABELS[value]}
              </option>
            ))}
          </select>
        </label>
      </div>
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.coaches.length === 0 && (
        <p className="text-sm">
          {filtered ? "条件に合うコーチはいません" : "自己紹介の足りないコーチはいません"}
        </p>
      )}
      {state.kind === "loaded" && state.coaches.length > 0 && (
        <table aria-label="自己紹介の足りないコーチの一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">コーチ</th>
              <th className="py-1">空の項目</th>
              <th className="py-1">最後に更新した日時</th>
            </tr>
          </thead>
          <tbody>
            {state.coaches.map((coach) => (
              <tr key={coach.coach_id} className="border-t">
                <td className="py-1">
                  <Link href={coachOverviewPath(coach.coach_id)} className="text-primary underline">
                    {coach.display_name}
                  </Link>
                </td>
                <td className="py-1">
                  {coach.missing_fields.map((field) => COACH_PROFILE_FIELD_LABELS[field]).join("、")}
                </td>
                <td className="py-1">{updatedAtLabel(coach)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
