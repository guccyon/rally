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
    recent_booking_count: 4,
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
    recent_booking_count: 0,
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
    recent_booking_count: 1,
  },
];

function listOf(menus: Schemas["CoachLessonMenu"][]) {
  return vi.fn<HeadquartersApi["listLessonMenus"]>(async () => ({
    ok: true,
    data: { lesson_menus: menus },
  }));
}

const COACHES: Schemas["Coach"][] = [
  { coach_id: COACH_A, display_name: "山田コーチ", acceptance_status: "accepting" },
  { coach_id: COACH_B, display_name: "佐藤コーチ", acceptance_status: "suspended" },
];

function apiOf(
  listLessonMenus: HeadquartersApi["listLessonMenus"],
  actions: Partial<
    Pick<HeadquartersApi, "listCoaches" | "unpublishLessonMenu" | "republishLessonMenu">
  > = {},
) {
  return {
    listLessonMenus,
    listCoaches: vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: true,
      data: { coaches: COACHES },
    })),
    unpublishLessonMenu: vi.fn<HeadquartersApi["unpublishLessonMenu"]>(),
    republishLessonMenu: vi.fn<HeadquartersApi["republishLessonMenu"]>(),
    ...actions,
  };
}

function cellsOf(row: HTMLElement) {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

describe("本部のレッスンメニュー", () => {
  test("開いたときに条件なしで読み、各行にスポーツ・コーチ・名前・種類・時間・公開中かを API の順で出す", async () => {
    const listLessonMenus = listOf(MENUS);
    render(<LessonMenuCatalog api={apiOf(listLessonMenus)} />);

    const table = await screen.findByRole("table", { name: "レッスンメニューの一覧" });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map(cellsOf)).toEqual([
      ["テニス", "山田コーチ", "フォアハンドの基礎", "同期（ビデオ通話）", "60 分", "4 件", "公開中", "非公開にする"],
      // コーチが自分で非公開にしたものも本部が非公開にできる
      ["テニス", "佐藤コーチ", "サーブのフォーム添削", "非同期（動画の添削）", "—", "0 件", "非公開", "非公開にする"],
      // コーチのアカウントが引けない行は表示名がない
      ["水泳", "（名前を表示できません）", "クロールの息継ぎ", "同期（ビデオ通話）", "45 分", "1 件", "公開中", "非公開にする"],
    ]);
    expect(listLessonMenus).toHaveBeenCalledTimes(1);
    expect(listLessonMenus).toHaveBeenCalledWith({});
    expect(screen.getByLabelText("スポーツ")).toHaveValue("");
    expect(screen.getByLabelText("公開中だけ")).not.toBeChecked();
  });

  test("直近 30 日の予約の数を「N 件」で右寄せ・等幅の数字で出し、表の下に数え方を書く", async () => {
    const menus = [{ ...MENUS[0]!, recent_booking_count: 128 }, MENUS[1]!];
    render(<LessonMenuCatalog api={apiOf(listOf(menus))} />);

    const table = await screen.findByRole("table", { name: "レッスンメニューの一覧" });
    const header = within(table).getByRole("columnheader", { name: "直近 30 日の予約" });
    expect(header).toHaveClass("text-right");
    const counts = within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("cell")[5]!);
    expect(counts.map((cell) => cell.textContent)).toEqual(["128 件", "0 件"]);
    for (const cell of counts) expect(cell).toHaveClass("text-right", "tabular-nums");
    expect(
      screen.getByText(
        "直近 30 日の予約は、今から 30 日前までに入った予約のうち、キャンセルされていないものの数です。",
      ),
    ).toBeInTheDocument();
  });

  test("読み込んでいる間は「読み込み中…」を出し、表と予約の数の説明は出さない", async () => {
    let resolve: (value: Awaited<ReturnType<HeadquartersApi["listLessonMenus"]>>) => void = () => {};
    const listLessonMenus = vi.fn<HeadquartersApi["listLessonMenus"]>(
      () => new Promise((done) => (resolve = done)),
    );
    render(<LessonMenuCatalog api={apiOf(listLessonMenus)} />);

    expect(await screen.findByText("読み込み中…")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(/直近 30 日の予約は/)).not.toBeInTheDocument();

    resolve({ ok: true, data: { lesson_menus: MENUS } });
    expect(await screen.findByRole("table")).toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("スポーツは「すべて」と 6 種類から選べる", async () => {
    render(<LessonMenuCatalog api={apiOf(listOf(MENUS))} />);
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
    render(<LessonMenuCatalog api={apiOf(listLessonMenus)} />);
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
    render(<LessonMenuCatalog api={apiOf(listLessonMenus)} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await user.selectOptions(screen.getByLabelText("スポーツ"), "水泳");
    await screen.findByRole("row", { name: /クロールの息継ぎ/ });
    resolveSlow({ ok: true, data: { lesson_menus: MENUS.slice(0, 2) } });

    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.queryByRole("row", { name: /フォアハンドの基礎/ })).not.toBeInTheDocument();
  });

  test("0 件なら「条件に合うメニューはありません」と出し、表は出さない", async () => {
    render(<LessonMenuCatalog api={apiOf(listOf([]))} />);

    expect(await screen.findByText("条件に合うメニューはありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(/直近 30 日の予約は/)).not.toBeInTheDocument();
    // 絞り込みは 0 件でも変えられる
    expect(screen.getByLabelText("スポーツ")).toBeEnabled();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listLessonMenus = vi.fn<HeadquartersApi["listLessonMenus"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<LessonMenuCatalog api={apiOf(listLessonMenus)} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("条件に合うメニューはありません")).not.toBeInTheDocument();
    expect(screen.queryByText(/直近 30 日の予約は/)).not.toBeInTheDocument();
  });
});

describe("本部のレッスンメニューをコーチで絞る", () => {
  test("コーチは「すべて」とコーチの一覧の返った順から選べ、選ぶと API の coach_id に渡して読み直す", async () => {
    const listLessonMenus = vi
      .fn<HeadquartersApi["listLessonMenus"]>()
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: MENUS } })
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: [MENUS[1]!] } })
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: [] } })
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: [MENUS[1]!] } })
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: MENUS } });
    const api = apiOf(listLessonMenus);
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={api} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toBeEnabled());

    const options = within(screen.getByLabelText("コーチ")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["すべて", "山田コーチ", "佐藤コーチ"]);
    expect(screen.getByLabelText("コーチ")).toHaveValue("");
    expect(api.listCoaches).toHaveBeenCalledWith();

    await user.selectOptions(screen.getByLabelText("コーチ"), "佐藤コーチ");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(listLessonMenus).toHaveBeenLastCalledWith({ coach_id: COACH_B });
    // 公開中も非公開もまとめて出る（非公開の行も残す）
    expect(cellsOf(rowOf("サーブのフォーム添削"))[6]).toBe("非公開");

    // ほかの絞り込みと重ねられる。0 件なら表を出さない
    await user.click(screen.getByLabelText("公開中だけ"));
    expect(await screen.findByText("条件に合うメニューはありません")).toBeInTheDocument();
    expect(listLessonMenus).toHaveBeenLastCalledWith({ published: true, coach_id: COACH_B });
    expect(screen.queryByRole("table")).not.toBeInTheDocument();

    await user.click(screen.getByLabelText("公開中だけ"));
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(listLessonMenus).toHaveBeenLastCalledWith({ coach_id: COACH_B });
    await user.selectOptions(screen.getByLabelText("コーチ"), "すべて");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(4));
    expect(listLessonMenus).toHaveBeenLastCalledWith({});
  });

  test("initialCoachId があれば、そのコーチで絞って開き、コーチを選んだ状態にする", async () => {
    const listLessonMenus = listOf([MENUS[0]!, MENUS[2]!]);
    render(<LessonMenuCatalog api={apiOf(listLessonMenus)} initialCoachId={COACH_A} />);

    await screen.findByRole("table");
    expect(listLessonMenus).toHaveBeenCalledTimes(1);
    expect(listLessonMenus).toHaveBeenCalledWith({ coach_id: COACH_A });
    await waitFor(() => expect(screen.getByLabelText("コーチ")).toHaveDisplayValue("山田コーチ"));
  });

  test("コーチの一覧にない initialCoachId でもそのまま API に渡し、0 件の表示と「（一覧にないコーチ）」を出す", async () => {
    const unknown = "44444444-4444-4444-8444-444444444444";
    const listLessonMenus = listOf([]);
    render(<LessonMenuCatalog api={apiOf(listLessonMenus)} initialCoachId={unknown} />);

    expect(await screen.findByText("条件に合うメニューはありません")).toBeInTheDocument();
    expect(listLessonMenus).toHaveBeenCalledWith({ coach_id: unknown });
    await waitFor(() =>
      expect(screen.getByLabelText("コーチ")).toHaveDisplayValue("（一覧にないコーチ）"),
    );
  });

  test("コーチの一覧を読めなければ文言を出し、メニューの一覧は出す（コーチの絞り込みは選べない）", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<LessonMenuCatalog api={apiOf(listOf(MENUS), { listCoaches })} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "コーチの一覧を読めませんでした。サーバーに接続できません。",
    );
    expect(await screen.findByRole("table")).toBeInTheDocument();
    expect(screen.getByLabelText("コーチ")).toBeDisabled();
  });

  test("メニューの一覧を読めなければ、コーチで絞っていてもその文言を出し、一覧は出さない", async () => {
    const listLessonMenus = vi.fn<HeadquartersApi["listLessonMenus"]>(async () => ({
      ok: false,
      message: "入力が正しくありません。",
    }));
    render(<LessonMenuCatalog api={apiOf(listLessonMenus)} initialCoachId={COACH_A} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("入力が正しくありません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});

const HQ_UNPUBLISHED: Schemas["CoachLessonMenu"] = {
  lesson_menu_id: "aaaaaaaa-0000-4000-8000-000000000004",
  name: "ボレーの基礎",
  sport: "tennis",
  lesson_type: "live_lesson",
  duration_minutes: 30,
  is_published: false,
  coach_id: COACH_A,
  coach_display_name: "山田コーチ",
  created_at: "2026-10-04T00:00:00Z",
  headquarters_unpublish: { unpublished_at: "2026-10-26T03:00:00Z", reason: "料金の表記が誤っている" },
  recent_booking_count: 0,
};

/** unpublishLessonMenu / republishLessonMenu の応答（LessonMenu。説明つき・表示名なし）。 */
function lessonMenuOf(
  menu: Schemas["CoachLessonMenu"],
  change: Pick<Schemas["LessonMenu"], "is_published" | "headquarters_unpublish">,
): Schemas["LessonMenu"] {
  return {
    lesson_menu_id: menu.lesson_menu_id,
    coach_id: menu.coach_id,
    sport: menu.sport,
    lesson_type: menu.lesson_type,
    name: menu.name,
    description: "",
    duration_minutes: menu.duration_minutes,
    created_at: menu.created_at,
    ...change,
  };
}

function rowOf(name: string) {
  return screen.getByRole("row", { name: new RegExp(name) });
}

describe("本部がレッスンメニューを非公開にする・公開に戻す", () => {
  test("本部が非公開にした行は「非公開（本部）」と非公開にした日時・理由を出し、操作は「公開に戻す」", async () => {
    render(<LessonMenuCatalog api={apiOf(listOf([HQ_UNPUBLISHED]))} />);

    const row = await screen.findByRole("row", { name: /ボレーの基礎/ });
    const cells = cellsOf(row);
    expect(cells[6]).toContain("非公開（本部）");
    expect(cells[6]).toContain("理由: 料金の表記が誤っている");
    expect(cells[6]).toContain("2026");
    expect(cells[7]).toBe("公開に戻す");
  });

  test("理由が空なら日時だけを出す", async () => {
    const menu = {
      ...HQ_UNPUBLISHED,
      headquarters_unpublish: { unpublished_at: "2026-10-26T03:00:00Z", reason: "" },
    };
    render(<LessonMenuCatalog api={apiOf(listOf([menu]))} />);

    const row = await screen.findByRole("row", { name: /ボレーの基礎/ });
    expect(cellsOf(row)[6]).not.toContain("理由");
  });

  test("「非公開にする」は行の中で確かめ、理由を付けて確定すると送り、送っている間は押せず、結果を行に反映する", async () => {
    let resolve: (value: Awaited<ReturnType<HeadquartersApi["unpublishLessonMenu"]>>) => void =
      () => {};
    const unpublishLessonMenu = vi.fn<HeadquartersApi["unpublishLessonMenu"]>(
      () => new Promise((r) => (resolve = r)),
    );
    const listLessonMenus = listOf(MENUS);
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={apiOf(listLessonMenus, { unpublishLessonMenu })} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "「フォアハンドの基礎」を非公開にする" }));
    const group = screen.getByRole("group", { name: "「フォアハンドの基礎」の非公開の確認" });
    expect(group).toHaveTextContent("本部が公開に戻すまでコーチは公開し直せません");
    expect(unpublishLessonMenu).not.toHaveBeenCalled();

    await user.type(within(group).getByLabelText("理由（任意）"), "誤った内容");
    await user.click(within(group).getByRole("button", { name: "非公開を確定" }));
    expect(unpublishLessonMenu).toHaveBeenCalledWith(MENUS[0]!.lesson_menu_id, {
      reason: "誤った内容",
    });
    expect(within(group).getByRole("button", { name: "非公開にしています…" })).toBeDisabled();
    expect(within(group).getByRole("button", { name: "やめる" })).toBeDisabled();

    resolve({
      ok: true,
      data: lessonMenuOf(MENUS[0]!, {
        is_published: false,
        headquarters_unpublish: { unpublished_at: "2026-10-26T03:00:00Z", reason: "誤った内容" },
      }),
    });

    expect(await screen.findByRole("status")).toHaveTextContent(
      "「フォアハンドの基礎」を非公開にしました。",
    );
    const cells = cellsOf(rowOf("フォアハンドの基礎"));
    // 表示名は応答（LessonMenu）にないので、一覧の行のものを残す
    expect(cells[1]).toBe("山田コーチ");
    expect(cells[6]).toContain("非公開（本部）");
    expect(cells[6]).toContain("理由: 誤った内容");
    expect(cells[7]).toBe("公開に戻す");
    // 一覧は取り直さない
    expect(listLessonMenus).toHaveBeenCalledTimes(1);
  });

  test("理由を書かなければ本文を空にして送る", async () => {
    const unpublishLessonMenu = vi.fn<HeadquartersApi["unpublishLessonMenu"]>(async () => ({
      ok: true,
      data: lessonMenuOf(MENUS[1]!, {
        is_published: false,
        headquarters_unpublish: { unpublished_at: "2026-10-26T03:00:00Z", reason: "" },
      }),
    }));
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={apiOf(listOf(MENUS), { unpublishLessonMenu })} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "「サーブのフォーム添削」を非公開にする" }));
    await user.click(screen.getByRole("button", { name: "非公開を確定" }));

    expect(unpublishLessonMenu).toHaveBeenCalledWith(MENUS[1]!.lesson_menu_id, {});
    await waitFor(() => expect(cellsOf(rowOf("サーブのフォーム添削"))[6]).toContain("非公開（本部）"));
  });

  test("「やめる」で送らずに戻り、書いた理由は残さない", async () => {
    const unpublishLessonMenu = vi.fn<HeadquartersApi["unpublishLessonMenu"]>();
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={apiOf(listOf(MENUS), { unpublishLessonMenu })} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "「フォアハンドの基礎」を非公開にする" }));
    await user.type(screen.getByLabelText("理由（任意）"), "下書き");
    await user.click(screen.getByRole("button", { name: "やめる" }));

    expect(unpublishLessonMenu).not.toHaveBeenCalled();
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "「フォアハンドの基礎」を非公開にする" }));
    expect(screen.getByLabelText("理由（任意）")).toHaveValue("");
  });

  test("理由は 200 文字（コードポイント）まで。超えたら確定を押せず、超えた文字数を出す", async () => {
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={apiOf(listOf(MENUS))} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "「フォアハンドの基礎」を非公開にする" }));
    const reason = screen.getByLabelText("理由（任意）");
    expect(screen.getByText("あと 200 文字")).toBeInTheDocument();

    // サロゲートペアの絵文字も 1 文字と数える
    await user.click(reason);
    await user.paste("🎾".repeat(200));
    expect(screen.getByText("あと 0 文字")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "非公開を確定" })).toBeEnabled();

    await user.paste("あ");
    expect(screen.getByText("200 文字を 1 文字超えています")).toBeInTheDocument();
    expect(reason).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("button", { name: "非公開を確定" })).toBeDisabled();
  });

  test("「公開に戻す」は確かめてから送り、行を公開中にして操作を「非公開にする」に戻す", async () => {
    const republishLessonMenu = vi.fn<HeadquartersApi["republishLessonMenu"]>(async () => ({
      ok: true,
      data: lessonMenuOf(HQ_UNPUBLISHED, { is_published: true }),
    }));
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={apiOf(listOf([HQ_UNPUBLISHED]), { republishLessonMenu })} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "「ボレーの基礎」を公開に戻す" }));
    const group = screen.getByRole("group", { name: "「ボレーの基礎」の公開に戻す確認" });
    expect(group).toHaveTextContent("生徒の一覧にまた出て");
    expect(republishLessonMenu).not.toHaveBeenCalled();
    await user.click(within(group).getByRole("button", { name: "公開に戻すのを確定" }));

    expect(republishLessonMenu).toHaveBeenCalledWith(HQ_UNPUBLISHED.lesson_menu_id);
    expect(await screen.findByRole("status")).toHaveTextContent("「ボレーの基礎」を公開に戻しました。");
    const cells = cellsOf(rowOf("ボレーの基礎"));
    expect(cells[6]).toBe("公開中");
    expect(cells[7]).toBe("非公開にする");
  });

  test("断られたら（409 など）ErrorCode の文言を出し、今の絞り込みで一覧を取り直す", async () => {
    const listLessonMenus = vi
      .fn<HeadquartersApi["listLessonMenus"]>()
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: MENUS } })
      .mockResolvedValueOnce({ ok: true, data: { lesson_menus: MENUS.slice(0, 2) } })
      .mockResolvedValueOnce({
        ok: true,
        data: {
          lesson_menus: [
            {
              ...MENUS[0]!,
              is_published: false,
              headquarters_unpublish: HQ_UNPUBLISHED.headquarters_unpublish!,
            },
          ],
        },
      });
    const unpublishLessonMenu = vi.fn<HeadquartersApi["unpublishLessonMenu"]>(async () => ({
      ok: false,
      code: "lesson_menu_already_unpublished_by_headquarters",
      message: "このメニューはすでに本部が非公開にしています。",
    }));
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={apiOf(listLessonMenus, { unpublishLessonMenu })} />);
    await screen.findByRole("table");
    await user.selectOptions(screen.getByLabelText("スポーツ"), "テニス");
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(3));

    await user.click(screen.getByRole("button", { name: "「フォアハンドの基礎」を非公開にする" }));
    await user.click(screen.getByRole("button", { name: "非公開を確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "このメニューはすでに本部が非公開にしています。",
    );
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(listLessonMenus).toHaveBeenCalledTimes(3);
    expect(listLessonMenus).toHaveBeenLastCalledWith({ sport: "tennis" });
    expect(cellsOf(rowOf("フォアハンドの基礎"))[7]).toBe("公開に戻す");
    // 文言は取り直した後も残す
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  test("サーバーに接続できないときは文言を出し、一覧は取り直さずに行もそのまま", async () => {
    const listLessonMenus = listOf([HQ_UNPUBLISHED]);
    const republishLessonMenu = vi.fn<HeadquartersApi["republishLessonMenu"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={apiOf(listLessonMenus, { republishLessonMenu })} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "「ボレーの基礎」を公開に戻す" }));
    await user.click(screen.getByRole("button", { name: "公開に戻すのを確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(listLessonMenus).toHaveBeenCalledTimes(1);
    expect(cellsOf(rowOf("ボレーの基礎"))[7]).toBe("公開に戻す");
  });

  test("絞り込みを変えたら前の操作の文言を消す", async () => {
    const unpublishLessonMenu = vi.fn<HeadquartersApi["unpublishLessonMenu"]>(async () => ({
      ok: true,
      data: lessonMenuOf(MENUS[0]!, {
        is_published: false,
        headquarters_unpublish: { unpublished_at: "2026-10-26T03:00:00Z", reason: "" },
      }),
    }));
    const user = userEvent.setup();
    render(<LessonMenuCatalog api={apiOf(listOf(MENUS), { unpublishLessonMenu })} />);
    await screen.findByRole("table");
    await user.click(screen.getByRole("button", { name: "「フォアハンドの基礎」を非公開にする" }));
    await user.click(screen.getByRole("button", { name: "非公開を確定" }));
    await screen.findByRole("status");

    await user.click(screen.getByLabelText("公開中だけ"));
    await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
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
