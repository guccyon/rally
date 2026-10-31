"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { ResultMessage } from "@/components/ResultMessage";
import { LESSON_TYPE_LABELS, SPORT_LABELS } from "@/components/labels";

type Sport = Schemas["Sport"];
type LessonMenu = Schemas["CoachLessonMenu"];

type State =
  | { kind: "loading" }
  | { kind: "loaded"; menus: LessonMenu[] }
  | { kind: "failed"; message: string };

type Filter = { sport: Sport | ""; publishedOnly: boolean };

const SPORTS = Object.keys(SPORT_LABELS) as Sport[];

function durationLabel(menu: LessonMenu): string {
  return menu.duration_minutes === null ? "—" : `${menu.duration_minutes} 分`;
}

/**
 * 本部の「レッスンメニュー」。すべてのコーチのメニューを横断で出す（listLessonMenus）。
 * 絞り込み（スポーツ・公開中だけ）は API の `sport` / `published` に渡し、並びは契約の正本どおり返った順のまま出す。
 */
export function LessonMenuCatalog({ api }: { api: Pick<HeadquartersApi, "listLessonMenus"> }) {
  const [filter, setFilter] = useState<Filter>({ sport: "", publishedOnly: false });
  const [state, setState] = useState<State>({ kind: "loading" });
  // 絞り込みを続けて変えたときに、前の条件の遅れて返った結果で上書きしないための番号。
  const latestRequest = useRef(0);

  const fetchMenus = useCallback(
    async (next: Filter): Promise<State> => {
      const response = await api.listLessonMenus({
        ...(next.sport === "" ? {} : { sport: next.sport }),
        ...(next.publishedOnly ? { published: true } : {}),
      });
      return response.ok
        ? { kind: "loaded", menus: response.data.lesson_menus }
        : { kind: "failed", message: response.message };
    },
    [api],
  );

  // 開いたときの読み込み（条件なし）。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    const request = ++latestRequest.current;
    void fetchMenus({ sport: "", publishedOnly: false }).then((next) => {
      if (active && request === latestRequest.current) setState(next);
    });
    return () => {
      active = false;
    };
  }, [fetchMenus]);

  async function changeFilter(next: Filter) {
    setFilter(next);
    const request = ++latestRequest.current;
    setState({ kind: "loading" });
    const result = await fetchMenus(next);
    if (request === latestRequest.current) setState(result);
  }

  return (
    <div className="space-y-4">
      <p className="text-sm">コーチが登録したレッスンメニューを、コーチをまたいで見られます。</p>
      <div className="flex flex-wrap items-end gap-4">
        <label className="block">
          スポーツ
          <select
            className="mt-1 block rounded border px-2 py-1"
            value={filter.sport}
            onChange={(event) =>
              void changeFilter({ ...filter, sport: event.target.value as Sport | "" })
            }
          >
            <option value="">すべて</option>
            {SPORTS.map((sport) => (
              <option key={sport} value={sport}>
                {SPORT_LABELS[sport]}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            checked={filter.publishedOnly}
            onChange={(event) =>
              void changeFilter({ ...filter, publishedOnly: event.target.checked })
            }
          />
          公開中だけ
        </label>
      </div>
      {state.kind === "loading" && <p className="text-sm text-gray-500">読み込み中…</p>}
      {state.kind === "failed" && <ResultMessage result={{ ok: false, message: state.message }} />}
      {state.kind === "loaded" && state.menus.length === 0 && (
        <p className="text-sm">条件に合うメニューはありません</p>
      )}
      {state.kind === "loaded" && state.menus.length > 0 && (
        <table aria-label="レッスンメニューの一覧" className="w-full text-left text-sm">
          <thead>
            <tr>
              <th className="py-1">スポーツ</th>
              <th className="py-1">コーチ</th>
              <th className="py-1">名前</th>
              <th className="py-1">種類</th>
              <th className="py-1">時間</th>
              <th className="py-1">公開</th>
            </tr>
          </thead>
          <tbody>
            {state.menus.map((menu) => (
              <tr key={menu.lesson_menu_id} className="border-t">
                <td className="py-1">{SPORT_LABELS[menu.sport]}</td>
                <td className="py-1">{menu.coach_display_name ?? "（名前を表示できません）"}</td>
                <td className="py-1">{menu.name}</td>
                <td className="py-1">{LESSON_TYPE_LABELS[menu.lesson_type]}</td>
                <td className="py-1">{durationLabel(menu)}</td>
                <td className="py-1">{menu.is_published ? "公開中" : "非公開"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
