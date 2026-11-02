import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { LessonMenuPublicationHistory } from "@/components/headquarters/LessonMenuPublicationHistory";
import { LessonMenuPublicationHistoryHome } from "@/components/headquarters/LessonMenuPublicationHistoryHome";
import { formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const LESSON_MENU_ID = "aaaaaaaa-0000-4000-8000-000000000001";
const HQ_A = "44444444-4444-4444-8444-00000000000a";
const HQ_B = "44444444-4444-4444-8444-00000000000b";

const CHANGES: Schemas["LessonMenuPublicationChangeList"] = {
  lesson_menu_id: LESSON_MENU_ID,
  changes: [
    { action: "republished", reason: null, changed_by: HQ_B, changed_at: "2026-10-30T01:00:00Z" },
    { action: "unpublished", reason: "料金の表記が誤っている", changed_by: HQ_A, changed_at: "2026-10-28T09:30:00Z" },
    { action: "unpublished", reason: "", changed_by: HQ_A, changed_at: "2026-10-20T00:00:00Z" },
  ],
};

function apiOf(result: Result<Schemas["LessonMenuPublicationChangeList"]>) {
  return {
    listLessonMenuPublicationChanges: vi.fn<HeadquartersApi["listLessonMenuPublicationChanges"]>(
      async () => result,
    ),
  };
}

describe("本部のレッスンメニューの公開の記録", () => {
  test("読み込み中を出し、読めたら返った順（新しい順）に操作・日時・操作した本部のアカウント・理由を出す", async () => {
    const api = apiOf({ ok: true, data: CHANGES });
    render(<LessonMenuPublicationHistory api={api} lessonMenuId={LESSON_MENU_ID} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "公開の記録" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(api.listLessonMenuPublicationChanges).toHaveBeenCalledTimes(1);
    expect(api.listLessonMenuPublicationChanges).toHaveBeenCalledWith(LESSON_MENU_ID);

    expect(within(table).getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual([
      "操作",
      "日時",
      "操作した本部のアカウント",
      "理由",
    ]);
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent))).toEqual([
      // 公開に戻した記録の理由は null
      ["公開に戻した", formatDateTime("2026-10-30T01:00:00Z"), HQ_B, "—"],
      ["非公開にした", formatDateTime("2026-10-28T09:30:00Z"), HQ_A, "料金の表記が誤っている"],
      // 理由を書かずに非公開にした記録は空文字列
      ["非公開にした", formatDateTime("2026-10-20T00:00:00Z"), HQ_A, "—"],
    ]);
  });

  test("同じ時刻の記録も返った順のまま出す", async () => {
    const at = "2026-10-30T01:00:00Z";
    const data: Schemas["LessonMenuPublicationChangeList"] = {
      lesson_menu_id: LESSON_MENU_ID,
      changes: [
        { action: "unpublished", reason: "", changed_by: HQ_A, changed_at: at },
        { action: "republished", reason: null, changed_by: HQ_B, changed_at: at },
      ],
    };
    render(<LessonMenuPublicationHistory api={apiOf({ ok: true, data })} lessonMenuId={LESSON_MENU_ID} />);

    const table = await screen.findByRole("table", { name: "公開の記録" });
    const firstCells = within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("cell")[0]!.textContent);
    expect(firstCells).toEqual(["非公開にした", "公開に戻した"]);
  });

  test("0 件なら「本部が非公開にしたことはありません」を出す", async () => {
    render(
      <LessonMenuPublicationHistory
        api={apiOf({ ok: true, data: { lesson_menu_id: LESSON_MENU_ID, changes: [] } })}
        lessonMenuId={LESSON_MENU_ID}
      />,
    );

    expect(await screen.findByText("本部が非公開にしたことはありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("ないメニュー（404）なら「見つかりません」を出す", async () => {
    render(
      <LessonMenuPublicationHistory
        api={apiOf({ ok: false, code: "not_found", message: "対象が見つかりません（ID を確かめてください）。" })}
        lessonMenuId={LESSON_MENU_ID}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "このレッスンメニューは見つかりません。レッスンメニューの一覧から選び直してください。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("メニューの ID がなければ API を呼ばずに「見つかりません」を出す", () => {
    const api = apiOf({ ok: true, data: CHANGES });
    render(<LessonMenuPublicationHistory api={api} lessonMenuId={undefined} />);

    expect(screen.getByRole("alert")).toHaveTextContent("このレッスンメニューは見つかりません。");
    expect(api.listLessonMenuPublicationChanges).not.toHaveBeenCalled();
  });

  test("ほかの失敗はその文言を出す", async () => {
    render(
      <LessonMenuPublicationHistory
        api={apiOf({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" })}
        lessonMenuId={LESSON_MENU_ID}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "公開の記録を読めませんでした。権限がないためアクセスできません。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("非公開にする・公開に戻す操作は出さない", async () => {
    render(<LessonMenuPublicationHistory api={apiOf({ ok: true, data: CHANGES })} lessonMenuId={LESSON_MENU_ID} />);

    await screen.findByRole("table", { name: "公開の記録" });
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});

describe("画面への配置", () => {
  test("「公開の記録」の欄と、レッスンメニューの一覧へ戻るリンクがある", () => {
    render(<LessonMenuPublicationHistoryHome token="mock.headquarters.abc" lessonMenuId={LESSON_MENU_ID} />);

    expect(screen.getByRole("region", { name: "公開の記録" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "レッスンメニューの一覧へ戻る" })).toHaveAttribute(
      "href",
      "/headquarters/lesson-menus",
    );
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <LessonMenuPublicationHistoryHome token={session.token} lessonMenuId={LESSON_MENU_ID} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "公開の記録" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
