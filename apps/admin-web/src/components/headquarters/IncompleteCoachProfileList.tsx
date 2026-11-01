"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { COACH_PROFILE_FIELD_LABELS, formatDateTime } from "@/components/labels";

import { coachOverviewPath } from "./CoachOverviewHome";

type IncompleteCoachProfile = Schemas["IncompleteCoachProfile"];

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
 */
export function IncompleteCoachProfileList({
  api,
}: {
  api: Pick<HeadquartersApi, "listIncompleteCoachProfiles">;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void api.listIncompleteCoachProfiles().then((response) => {
      if (!active) return;
      setState(
        response.ok
          ? { kind: "loaded", coaches: response.data.coaches }
          : { kind: "failed", message: response.message },
      );
    });
    return () => {
      active = false;
    };
  }, [api]);

  return (
    <div className="space-y-4">
      <p className="text-sm">
        自己紹介に空の項目があるコーチを、登録の古い順に見られます。書きかけのままのコーチを見つけて声をかけるための一覧です。
      </p>
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.coaches.length === 0 && (
        <p className="text-sm">自己紹介の足りないコーチはいません</p>
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
                  <Link href={coachOverviewPath(coach.coach_id)} className="text-blue-600 underline">
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
