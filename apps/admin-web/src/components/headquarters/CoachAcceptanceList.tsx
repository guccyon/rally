"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";

import { CoachAcceptanceAction } from "./CoachAcceptanceAction";
import { CoachAcceptanceBadge } from "./CoachAcceptanceBadge";
import { coachOverviewPath } from "./CoachOverviewHome";

type Coach = Schemas["Coach"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; coaches: Coach[] }
  | { kind: "failed"; message: string };

/**
 * 本部の「コーチの受付」。開いたときにコーチの一覧（listCoaches）を読み、各行に表示名と受付の状態を出す。
 * 受付中の行は「受付を止める」（suspendCoach）、停止中の行は「再開する」（resumeCoach）を行の中で確かめてから送り、
 * 応答の状態を行に入れる。並びは契約の正本どおり返った順（表示名の順）のまま出す。表示名からそのコーチの様子へ移る。
 */
export function CoachAcceptanceList({
  api,
}: {
  api: Pick<HeadquartersApi, "listCoaches" | "suspendCoach" | "resumeCoach">;
}) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);

  const fetchCoaches = useCallback(async (): Promise<State> => {
    const response = await api.listCoaches();
    return response.ok
      ? { kind: "loaded", coaches: response.data.coaches }
      : { kind: "failed", message: response.message };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchCoaches().then((next) => {
      if (active) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchCoaches]);

  async function changeAcceptance(coach: Coach) {
    const suspending = coach.acceptance_status === "accepting";
    const response = suspending
      ? await api.suspendCoach(coach.coach_id)
      : await api.resumeCoach(coach.coach_id);
    if (response.ok) {
      setResult({
        ok: true,
        message: suspending
          ? `${coach.display_name}の受付を止めました。`
          : `${coach.display_name}の受付を再開しました。`,
      });
      const status = response.data.acceptance_status;
      setState((current) =>
        current.kind === "loaded"
          ? {
              ...current,
              coaches: current.coaches.map((row) =>
                row.coach_id === coach.coach_id ? { ...row, acceptance_status: status } : row,
              ),
            }
          : current,
      );
      return;
    }
    // 409（読み込んだ後に他で止めた・再開した）や 404 などはサーバーの ErrorCode の文言を出し、一覧を今の状態に取り直す。
    setResult(response);
    if (response.code !== undefined) setState(await fetchCoaches());
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">
        受付を止めたコーチの枠は生徒の検索に出ず、新しい予約を受け付けません。入っている予約と枠はそのままです。
      </p>
      <ResultMessage result={result} />
      {state.kind === "loading" && <p className="text-sm text-text-muted">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.coaches.length === 0 && (
        <p className="text-sm">コーチがまだいません</p>
      )}
      {state.kind === "loaded" && state.coaches.length > 0 && (
        <table aria-label="コーチの一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">表示名</th>
              <th className="py-1">受付</th>
              <th className="py-1">操作</th>
            </tr>
          </thead>
          <tbody>
            {state.coaches.map((coach) => (
              <tr key={coach.coach_id} className="border-t align-top">
                <td className="py-1">
                  <Link href={coachOverviewPath(coach.coach_id)} className="text-primary underline">
                    {coach.display_name}
                  </Link>
                </td>
                <td className="py-1">
                  <CoachAcceptanceBadge status={coach.acceptance_status} />
                </td>
                <td className="py-1">
                  <CoachAcceptanceAction
                    // 状態が変わったら確かめ中の表示を捨て、新しい状態のボタンから出し直す。
                    key={coach.acceptance_status}
                    status={coach.acceptance_status}
                    onConfirm={() => changeAcceptance(coach)}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
