import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { LessonMenuCatalog } from "@/components/headquarters/LessonMenuCatalog";
import { LessonMenuCatalogHome } from "@/components/headquarters/LessonMenuCatalogHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const COACH_A = "22222222-2222-4222-8222-222222222222";
const COACH_B = "33333333-3333-4333-8333-333333333333";

const MENUS: Schemas["CoachLessonMenu"][] = [
  {
    lesson_menu_id: "aaaaaaaa-0000-4000-8000-000000000001",
    name: "フォアハンドの基礎",
    sport: "tennis",
    lesson_type: "live_lesson",
    duration_minutes: 60,
    is_published: true,
    coach_id: COACH_A,
    coach_display_name: "山田コーチ",
    created_at: "2026-10-01T00:00:00Z",
  },
  {
    lesson_menu_id: "aaaaaaaa-0000-4000-8000-000000000002",
    name: "サーブのフォーム添削",
    sport: "tennis",
    lesson_type: "async_lesson",
    duration_minutes: null,
    is_published: false,
    coach_id: COACH_B,
    coach_display_name: "佐藤コーチ",
    created_at: "2026-10-02T00:00:00Z",
  },
  {
    lesson_menu_id: "aaaaaaaa-0000-4000-8000-000000000003",
    name: "クロールの息継ぎ",
    sport: "swimming",
    lesson_type: "live_lesson",
    duration_minutes: 45,
    is_published: true,
    coach_id: COACH_A,
    created_at: "2026-10-03T00:00:00Z",
  },
];

function listOf(menus: Schemas["CoachLessonMenu"][]) {
  return vi.fn<HeadquartersApi["listLessonMenus"]>(async () => ({
    ok: true,
    data: { lesson_menus: menus },
  }));
}

function cellsOf(row: HTMLElement) {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

describe("本部のレッスンメニュー", () => {
  test("開いたときに条件なしで読み、各行にスポーツ・コーチ・名前・種類・時間・公開中かを API の順で出す", async () => {
    const listLessonMenus = listOf(MENUS);
    render(<LessonMenuCatalog api={{ listLessonMenus }} />);

    const table = await screen.findByRole("table", { name: "レッスンメニューの一覧" });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map(cellsOf)).toEqual([
      ["テニス", "山田コーチ", "フォアハンドの基礎", "同期（ビデオ通話）", "60 分", "公開中"],
      ["テニス", "佐藤コーチ", "サーブのフォーム添削", "非同期（動画の添削）", "—", "非公開"],
      // コーチのアカウントが引けない行は表示名がない
      ["水泳", "（名前を表示できません）", "クロールの息継ぎ", "同期（ビデオ通話）", "45 分", "公開中"],
    ]);
    expect(listLessonMenus).toHaveBeenCalledTimes(1);
    expect(listLessonMenus).toHaveBeenCalledWith({});
    expect(screen.getByLabelText("スポーツ")).toHaveValue("");
    expect(screen.getByLabelText("公開中だけ")).not.toBeChecked();
  });

  test("スポーツは「すべて」と 6 種類から選べる", async () => {
    render(<LessonMenuCatalog api={{ listLessonMenus: listOf(MENUS) }} />);
    await screen.findByRole("table");

    const options = within(screen.getByLabelText("スポーツ")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "すべて",
      "テニス",
      "ゴルフ",
      "野球",
      "サッカー",
      "バスケットボール",
      "水泳",
    ]);
  });

  test("スポーツと「公開中だけ」の絞り込みを API の sport / published に渡して読み直す", async () => {
    const listLessonMenus = vi
      .fn<HeadquartersApi["listLessonMenus"]>()
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: MENUS } })
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: MENUS.slice(0, 2) } })
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: MENUS.slice(0, 1) } })
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: [MENUS[0]!, MENUS[2]!] } });
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={{ listLessonMenus }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(3));
    expect(listLessonMenus).toHaveBeenLastCalledWith({ sport: "tennis" });

    await user.click(screen.getByLabelText("公開中だけ"));
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(listLessonMenus).toHaveBeenLastCalledWith({ sport: "tennis", published: true });
    expect(screen.getByRole("row", { name: /フォアハンドの基礎/ })).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("スポーツ"), "すべて");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(3));
    expect(listLessonMenus).toHaveBeenLastCalledWith({ published: true });
    expect(listLessonMenus).toHaveBeenCalledTimes(4);
  });

  test("後から変えた条件の結果だけを出す（前の条件の遅れた結果で上書きしない）", async () => {
    let resolveSlow: (value: Awaited<ReturnType<HeadquartersApi["listLessonMenus"]>>) => void =
      () => {};
    const listLessonMenus = vi
      .fn<HeadquartersApi["listLessonMenus"]>()
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: MENUS } })
      .mockImplementationOnce(() => new Promise((resolve) => (resolveSlow = resolve)))
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: [MENUS[2]!] } });
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={{ listLessonMenus }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await user.selectOptions(screen.getByLabelText("スポーツ"), "水泳");
    await screen.findByRole("row", { name: /クロールの息継ぎ/ });
    resolveSlow({ ok: true, data: { lesson_menus: MENUS.slice(0, 2) } });

    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.queryByRole("row", { name: /フォアハンドの基礎/ })).not.toBeInTheDocument();
  });

  test("0 件なら「条件に合うメニューはありません」と出し、表は出さない", async () => {
    render(<LessonMenuCatalog api={{ listLessonMenus: listOf([]) }} />);

    expect(await screen.findByText("条件に合うメニューはありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    // 絞り込みは 0 件でも変えられる
    expect(screen.getByLabelText("スポーツ")).toBeEnabled();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listLessonMenus = vi.fn<HeadquartersApi["listLessonMenus"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<LessonMenuCatalog api={{ listLessonMenus }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("条件に合うメニューはありません")).not.toBeInTheDocument();
  });
});

describe("画面への配置", () => {
  test("本部の画面からレッスンメニューの画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "コーチのメニューを見る" })).toHaveAttribute(
      "href",
      "/headquarters/lesson-menus",
    );
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <LessonMenuCatalogHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "レッスンメニュー" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
