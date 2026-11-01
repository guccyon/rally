"use client";

import { useCallback, useEffect, useState } from "react";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { errorMessage } from "@/api/errorMessage";
import { ResultMessage } from "@/components/ResultMessage";
import { LESSON_TYPE_LABELS, SPORT_LABELS } from "@/components/labels";

import { LessonMenuForm } from "./LessonMenuForm";
import { emptyInput, inputOf, type LessonMenuInput, toRequest } from "./lessonMenuInput";

type LessonMenu = Schemas["LessonMenu"];

type ListState =
  | { kind: "loading" }
  | { kind: "loaded"; menus: LessonMenu[] }
  | { kind: "failed"; message: string };

/** フォームの開き方。作成の欄は `key` を変えて開き直し、送れたら空にする。 */
type Editing = { kind: "create"; key: number } | { kind: "edit"; menu: LessonMenu };

// 画面で名前・説明・時間を確かめてから送るので、送ったあとの 422 は残りの規則（スポーツが本人の提供スポーツか）で
// 断られたものと読み、どう直せばよいかを添える。admin-web の契約にはコーチの提供スポーツを読む API がない。
const VALIDATION_FAILED = errorMessage({ code: "validation_failed" });
const SPORT_HINT = "スポーツがあなたの提供しているスポーツか確かめてください。";

function failureMessage(message: string): string {
  return message === VALIDATION_FAILED ? `${message}${SPORT_HINT}` : message;
}

function durationLabel(menu: LessonMenu): string {
  return menu.duration_minutes === null ? "—" : `${menu.duration_minutes} 分`;
}

/**
 * コーチの「レッスンメニュー」。開いたときに自分のメニューを公開中かを問わず API の順（作った時刻の古い順）で出し
 * （listMyLessonMenus）、作成（createMyLessonMenu）と、行ごとの編集・非公開にする／公開する（updateMyLessonMenu）を置く。
 * 作る・直すたびに一覧を取り直す。
 */
export function LessonMenus({
  api,
}: {
  api: Pick<CoachApi, "listMyLessonMenus" | "createMyLessonMenu" | "updateMyLessonMenu">;
}) {
  const [list, setList] = useState<ListState>({ kind: "loading" });
  const [editing, setEditing] = useState<Editing>({ kind: "create", key: 0 });
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const fetchMenus = useCallback(async (): Promise<ListState> => {
    const response = await api.listMyLessonMenus();
    if (!response.ok) return { kind: "failed", message: response.message };
    return { kind: "loaded", menus: response.data.lesson_menus };
  }, [api]);

  // 開いたときの読み込み。初期の状態が loading なので、ここでは結果だけを入れる。
  useEffect(() => {
    let active = true;
    void fetchMenus().then((next) => {
      if (active) setList(next);
    });
    return () => {
      active = false;
    };
  }, [fetchMenus]);

  async function save(input: LessonMenuInput) {
    setBusy(true);
    const body = toRequest(input);
    const response =
      editing.kind === "edit"
        ? await api.updateMyLessonMenu(editing.menu.lesson_menu_id, body)
        : await api.createMyLessonMenu(body);
    if (response.ok) {
      setResult({
        ok: true,
        message:
          editing.kind === "edit"
            ? `「${response.data.name}」を保存しました。`
            : `「${response.data.name}」を登録しました。`,
      });
      setEditing({ kind: "create", key: editing.kind === "create" ? editing.key + 1 : 0 });
      setList(await fetchMenus());
    } else {
      setResult({ ok: false, message: failureMessage(response.message) });
    }
    setBusy(false);
  }

  async function setPublished(menu: LessonMenu, isPublished: boolean) {
    setBusy(true);
    const response = await api.updateMyLessonMenu(
      menu.lesson_menu_id,
      toRequest({ ...inputOf(menu), isPublished }),
    );
    if (response.ok) {
      setResult({
        ok: true,
        message: isPublished
          ? `「${menu.name}」を公開しました。`
          : `「${menu.name}」を非公開にしました（生徒には見えません）。`,
      });
      // 編集中のメニューなら、古い中身のまま保存しないよう作成の欄に戻す。
      if (editing.kind === "edit" && editing.menu.lesson_menu_id === menu.lesson_menu_id) {
        setEditing({ kind: "create", key: 0 });
      }
      setList(await fetchMenus());
    } else {
      setResult({ ok: false, message: response.message });
    }
    setBusy(false);
  }

  // 作成の欄のスポーツの既定は、登録済みのメニューのスポーツ（本人の提供スポーツと分かっているもの）。
  const defaultSport = (list.kind === "loaded" && list.menus[0]?.sport) || "tennis";

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <h3 className="font-semibold">
          {editing.kind === "edit" ? `「${editing.menu.name}」を編集` : "メニューを登録"}
        </h3>
        {editing.kind === "edit" ? (
          <LessonMenuForm
            key={editing.menu.lesson_menu_id}
            initial={inputOf(editing.menu)}
            submitLabel="変更を保存"
            busy={busy}
            onSubmit={(input) => void save(input)}
            onCancel={() => {
              setEditing({ kind: "create", key: 0 });
              setResult(null);
            }}
          />
        ) : (
          <LessonMenuForm
            key={`create-${editing.key}-${defaultSport}`}
            initial={emptyInput(defaultSport)}
            submitLabel="登録する"
            busy={busy}
            onSubmit={(input) => void save(input)}
          />
        )}
      </div>
      <ResultMessage result={result} />
      <div className="space-y-2">
        <h3 className="font-semibold">自分のメニュー</h3>
        {list.kind === "loading" && <p className="text-sm text-muted">読み込み中…</p>}
        {list.kind === "failed" && <ResultMessage result={{ ok: false, message: list.message }} />}
        {list.kind === "loaded" && list.menus.length === 0 && (
          <p className="text-sm">まだレッスンメニューを登録していません。</p>
        )}
        {list.kind === "loaded" && list.menus.length > 0 && (
          <table aria-label="自分のメニュー" className="w-full text-left text-sm">
            <thead>
              <tr>
                <th className="py-1">スポーツ</th>
                <th className="py-1">種類</th>
                <th className="py-1">名前</th>
                <th className="py-1">時間</th>
                <th className="py-1">公開</th>
                <th className="py-1">操作</th>
              </tr>
            </thead>
            <tbody>
              {list.menus.map((menu) => (
                <tr key={menu.lesson_menu_id} className="border-t">
                  <td className="py-1">{SPORT_LABELS[menu.sport]}</td>
                  <td className="py-1">{LESSON_TYPE_LABELS[menu.lesson_type]}</td>
                  <td className="py-1">{menu.name}</td>
                  <td className="py-1">{durationLabel(menu)}</td>
                  <td className="py-1">{menu.is_published ? "公開中" : "非公開"}</td>
                  <td className="space-x-2 py-1">
                    <button
                      type="button"
                      disabled={busy}
                      aria-label={`「${menu.name}」を編集`}
                      onClick={() => {
                        setEditing({ kind: "edit", menu });
                        setResult(null);
                      }}
                      className="rounded border px-2 disabled:opacity-50"
                    >
                      編集
                    </button>
                    <button
                      type="button"
                      disabled={busy}
                      aria-label={
                        menu.is_published
                          ? `「${menu.name}」を非公開にする`
                          : `「${menu.name}」を公開する`
                      }
                      onClick={() => void setPublished(menu, !menu.is_published)}
                      className="rounded border px-2 disabled:opacity-50"
                    >
                      {menu.is_published ? "非公開にする" : "公開する"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
