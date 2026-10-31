import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { CoachHome } from "@/components/coach/CoachHome";
import { LessonMenuForm } from "@/components/coach/LessonMenuForm";
import { LessonMenus } from "@/components/coach/LessonMenus";
import { emptyInput, toRequest, validateInput } from "@/components/coach/lessonMenuInput";

import { EMPTY_LESSON_STATS } from "./lessonStatsFixtures";

// 配置のテストでは CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

const COACH_ID = "22222222-2222-4222-8222-222222222222";

function menu(id: string, overrides: Partial<Schemas["LessonMenu"]> = {}): Schemas["LessonMenu"] {
  return {
    lesson_menu_id: id,
    coach_id: COACH_ID,
    sport: "soccer",
    lesson_type: "live_lesson",
    name: "リフティング入門",
    description: "",
    duration_minutes: 30,
    is_published: true,
    created_at: "2026-10-30T01:00:00Z",
    ...overrides,
  };
}

// API の並び（作った時刻の古い順）。
const LIVE = menu("a0000000-0000-4000-8000-000000000001", {
  name: "フォーム改善 60 分",
  description: "ストロークを見ます",
  sport: "tennis",
  duration_minutes: 60,
});
const ASYNC = menu("a0000000-0000-4000-8000-000000000002", {
  name: "動画でスイング診断",
  sport: "tennis",
  lesson_type: "async_lesson",
  duration_minutes: null,
  is_published: false,
});

type MenuApi = Pick<CoachApi, "listMyLessonMenus" | "createMyLessonMenu" | "updateMyLessonMenu">;

function listOf(...lists: Schemas["LessonMenu"][][]) {
  const fn = vi.fn<CoachApi["listMyLessonMenus"]>();
  for (const lessonMenus of lists) {
    fn.mockResolvedValueOnce({ ok: true, data: { coach_id: COACH_ID, lesson_menus: lessonMenus } });
  }
  return fn;
}

function mockApi(overrides: Partial<MenuApi> = {}): MenuApi {
  return {
    listMyLessonMenus: listOf([LIVE, ASYNC], [LIVE, ASYNC]),
    createMyLessonMenu: vi.fn<CoachApi["createMyLessonMenu"]>(async (body) => ({
      ok: true,
      data: menu("a0000000-0000-4000-8000-000000000003", {
        ...body,
        description: body.description ?? "",
        duration_minutes: body.duration_minutes ?? null,
        is_published: body.is_published ?? true,
      }),
    })),
    updateMyLessonMenu: vi.fn<CoachApi["updateMyLessonMenu"]>(async (id, body) => ({
      ok: true,
      data: menu(id, body),
    })),
    ...overrides,
  };
}

beforeEach(() => {
  coachApiMock.mockReset();
});

describe("レッスンメニューの入力の検査", () => {
  const valid = { ...emptyInput("tennis"), name: "フォーム改善" };

  test("名前 1〜40 文字・説明 0〜400 文字・同期は 15〜120 分の 15 分刻みなら通す", () => {
    expect(validateInput(valid)).toEqual({});
    expect(
      validateInput({ ...valid, name: "🎾".repeat(40), description: "あ".repeat(400) }),
    ).toEqual({});
    expect(validateInput({ ...valid, durationMinutes: 15 })).toEqual({});
    expect(validateInput({ ...valid, durationMinutes: 120 })).toEqual({});
  });

  test("名前が空・空白だけ・41 文字以上なら断る（コードポイントで数える）", () => {
    expect(validateInput({ ...valid, name: "" }).name).toBe("名前を入力してください。");
    expect(validateInput({ ...valid, name: " 　" }).name).toBe("名前を入力してください。");
    expect(validateInput({ ...valid, name: "🎾".repeat(41) }).name).toBe(
      "名前は 40 文字までです。1 文字減らしてください。",
    );
  });

  test("説明が 401 文字以上なら断る", () => {
    expect(validateInput({ ...valid, description: "あ".repeat(402) }).description).toBe(
      "説明は 400 文字までです。2 文字減らしてください。",
    );
  });

  test("同期で時間が範囲の外か 15 分刻みでなければ断り、非同期では時間を見ない", () => {
    for (const minutes of [0, 10, 135, 50]) {
      expect(validateInput({ ...valid, durationMinutes: minutes }).durationMinutes).toBeDefined();
    }
    expect(validateInput({ ...valid, lessonType: "async_lesson", durationMinutes: 50 })).toEqual(
      {},
    );
  });

  test("契約の本文は名前と説明を書いたとおりに送り、非同期の時間は null にする", () => {
    expect(toRequest({ ...valid, name: " 診断 ", lessonType: "async_lesson" })).toEqual({
      sport: "tennis",
      lesson_type: "async_lesson",
      name: " 診断 ",
      description: "",
      duration_minutes: null,
      is_published: true,
    });
  });
});

describe("レッスンメニューのフォーム", () => {
  test("同期のときだけ 1 回の時間（15〜120 分の 15 分刻み）を選ばせる", async () => {
    const user = userEvent.setup();
    render(
      <LessonMenuForm
        initial={emptyInput("tennis")}
        submitLabel="登録する"
        busy={false}
        onSubmit={vi.fn()}
      />,
    );

    const duration = screen.getByLabelText("1 回の時間");
    expect(
      within(duration)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual(["15 分", "30 分", "45 分", "60 分", "75 分", "90 分", "105 分", "120 分"]);
    expect(duration).toHaveValue("60");

    await user.selectOptions(screen.getByLabelText("レッスンの種類"), "async_lesson");
    expect(screen.queryByLabelText("1 回の時間")).not.toBeInTheDocument();
  });

  test("名前が空なら送らずに項目の下へ理由を出す", async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <LessonMenuForm
        initial={emptyInput("tennis")}
        submitLabel="登録する"
        busy={false}
        onSubmit={onSubmit}
      />,
    );

    await user.type(screen.getByLabelText("名前"), "   ");
    await user.click(screen.getByRole("button", { name: "登録する" }));

    expect(onSubmit).not.toHaveBeenCalled();
    expect(screen.getByText("名前を入力してください。")).toBeInTheDocument();
    expect(screen.getByLabelText("名前")).toHaveAttribute("aria-invalid", "true");
  });

  test("名前・説明が長すぎれば超えた数を出し、送らない", async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <LessonMenuForm
        initial={emptyInput("tennis")}
        submitLabel="登録する"
        busy={false}
        onSubmit={onSubmit}
      />,
    );

    fireEvent.change(screen.getByLabelText("名前"), { target: { value: "あ".repeat(43) } });
    fireEvent.change(screen.getByLabelText("説明（任意）"), {
      target: { value: "い".repeat(401) },
    });
    expect(screen.getByText("40 文字を 3 文字超えています")).toBeInTheDocument();
    expect(screen.getByText("400 文字を 1 文字超えています")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "登録する" }));

    expect(onSubmit).not.toHaveBeenCalled();
    expect(
      screen.getByText("名前は 40 文字までです。3 文字減らしてください。"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("説明は 400 文字までです。1 文字減らしてください。"),
    ).toBeInTheDocument();
  });

  test("正しい入力なら入力をそのまま渡す", async () => {
    const onSubmit = vi.fn();
    const user = userEvent.setup();
    render(
      <LessonMenuForm
        initial={emptyInput("tennis")}
        submitLabel="登録する"
        busy={false}
        onSubmit={onSubmit}
      />,
    );

    await user.selectOptions(screen.getByLabelText("スポーツ"), "swimming");
    await user.selectOptions(screen.getByLabelText("1 回の時間"), "90");
    await user.type(screen.getByLabelText("名前"), "クロール矯正");
    await user.click(screen.getByLabelText("公開する（公開中のメニューだけが生徒に見えます）"));
    await user.click(screen.getByRole("button", { name: "登録する" }));

    expect(onSubmit).toHaveBeenCalledWith({
      sport: "swimming",
      lessonType: "live_lesson",
      name: "クロール矯正",
      description: "",
      durationMinutes: 90,
      isPublished: false,
    });
  });
});

describe("レッスンメニュー", () => {
  test("開いたときに自分のメニューを API の順で、スポーツ・種類・名前・時間・公開中かとともに出す", async () => {
    const api = mockApi();
    render(<LessonMenus api={api} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "自分のメニュー" });
    const [, first, second] = within(table).getAllByRole("row") as HTMLElement[];
    const rows = [first!, second!];
    expect(
      rows.map((row) =>
        within(row)
          .getAllByRole("cell")
          .slice(0, 5)
          .map((c) => c.textContent),
      ),
    ).toEqual([
      ["テニス", "同期（ビデオ通話）", "フォーム改善 60 分", "60 分", "公開中"],
      ["テニス", "非同期（動画の添削）", "動画でスイング診断", "—", "非公開"],
    ]);
    expect(
      within(first!).getByRole("button", { name: "「フォーム改善 60 分」を非公開にする" }),
    ).toBeInTheDocument();
    expect(
      within(second!).getByRole("button", { name: "「動画でスイング診断」を公開する" }),
    ).toBeInTheDocument();
  });

  test("0 件ならその旨を、一覧の API が失敗したらその文言を出す", async () => {
    const { unmount } = render(<LessonMenus api={mockApi({ listMyLessonMenus: listOf([]) })} />);
    expect(await screen.findByText("まだレッスンメニューを登録していません。")).toBeInTheDocument();
    unmount();

    render(
      <LessonMenus
        api={mockApi({
          listMyLessonMenus: vi.fn<CoachApi["listMyLessonMenus"]>(async () => ({
            ok: false,
            message: "権限がないためアクセスできません。",
          })),
        })}
      />,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "権限がないためアクセスできません。",
    );
  });

  test("作成: 入力を createMyLessonMenu に送り、登録した旨を出して一覧を取り直し、欄を空にする", async () => {
    const created = menu("a0000000-0000-4000-8000-000000000003", {
      name: "動画で添削",
      lesson_type: "async_lesson",
      duration_minutes: null,
    });
    const api = mockApi({ listMyLessonMenus: listOf([LIVE], [LIVE, created]) });
    const user = userEvent.setup();
    render(<LessonMenus api={api} />);
    await screen.findByRole("table", { name: "自分のメニュー" });

    // スポーツの既定は、登録済みのメニューのスポーツ（本人の提供スポーツと分かっているもの）。
    expect(screen.getByLabelText("スポーツ")).toHaveValue("tennis");
    await user.selectOptions(screen.getByLabelText("スポーツ"), "soccer");
    await user.selectOptions(screen.getByLabelText("レッスンの種類"), "async_lesson");
    await user.type(screen.getByLabelText("名前"), "動画で添削");
    await user.type(screen.getByLabelText("説明（任意）"), "ドリブルの動画を送ってください");
    await user.click(screen.getByRole("button", { name: "登録する" }));

    expect(api.createMyLessonMenu).toHaveBeenCalledWith({
      sport: "soccer",
      lesson_type: "async_lesson",
      name: "動画で添削",
      description: "ドリブルの動画を送ってください",
      duration_minutes: null,
      is_published: true,
    });
    expect(await screen.findByRole("status")).toHaveTextContent("「動画で添削」を登録しました。");
    expect(api.listMyLessonMenus).toHaveBeenCalledTimes(2);
    expect(await screen.findByRole("cell", { name: "動画で添削" })).toBeInTheDocument();
    expect(screen.getByLabelText("名前")).toHaveValue("");
  });

  test("作成: 画面の検査で断った入力は API に送らない", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<LessonMenus api={api} />);
    await screen.findByRole("table", { name: "自分のメニュー" });

    await user.click(screen.getByRole("button", { name: "登録する" }));

    expect(api.createMyLessonMenu).not.toHaveBeenCalled();
    expect(screen.getByText("名前を入力してください。")).toBeInTheDocument();
  });

  test("作成: サーバーが断った（422）ら、提供スポーツを確かめるよう添えて出し、入力は残す", async () => {
    const api = mockApi({
      createMyLessonMenu: vi.fn<CoachApi["createMyLessonMenu"]>(async () => ({
        ok: false,
        message: "入力の内容が正しくありません。",
      })),
    });
    const user = userEvent.setup();
    render(<LessonMenus api={api} />);
    await screen.findByRole("table", { name: "自分のメニュー" });

    await user.selectOptions(screen.getByLabelText("スポーツ"), "golf");
    await user.type(screen.getByLabelText("名前"), "パター");
    await user.click(screen.getByRole("button", { name: "登録する" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "入力の内容が正しくありません。スポーツがあなたの提供しているスポーツか確かめてください。",
    );
    expect(screen.getByLabelText("名前")).toHaveValue("パター");
    expect(screen.getByLabelText("スポーツ")).toHaveValue("golf");
  });

  test("編集: 行の「編集」で今の中身を欄に出し、丸ごと updateMyLessonMenu に送る", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<LessonMenus api={api} />);
    await screen.findByRole("table", { name: "自分のメニュー" });

    await user.click(screen.getByRole("button", { name: "「フォーム改善 60 分」を編集" }));
    expect(
      screen.getByRole("heading", { name: "「フォーム改善 60 分」を編集" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("名前")).toHaveValue("フォーム改善 60 分");
    expect(screen.getByLabelText("説明（任意）")).toHaveValue("ストロークを見ます");
    expect(screen.getByLabelText("1 回の時間")).toHaveValue("60");

    await user.clear(screen.getByLabelText("名前"));
    await user.type(screen.getByLabelText("名前"), "フォーム改善 45 分");
    await user.selectOptions(screen.getByLabelText("1 回の時間"), "45");
    await user.click(screen.getByRole("button", { name: "変更を保存" }));

    expect(api.updateMyLessonMenu).toHaveBeenCalledWith(LIVE.lesson_menu_id, {
      sport: "tennis",
      lesson_type: "live_lesson",
      name: "フォーム改善 45 分",
      description: "ストロークを見ます",
      duration_minutes: 45,
      is_published: true,
    });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "「フォーム改善 45 分」を保存しました。",
    );
    expect(api.listMyLessonMenus).toHaveBeenCalledTimes(2);
    // 保存したら作成の欄に戻る。
    expect(screen.getByRole("heading", { name: "メニューを登録" })).toBeInTheDocument();
  });

  test("編集: 「やめる」で送らずに作成の欄に戻る", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<LessonMenus api={api} />);
    await screen.findByRole("table", { name: "自分のメニュー" });

    await user.click(screen.getByRole("button", { name: "「動画でスイング診断」を編集" }));
    expect(screen.queryByLabelText("1 回の時間")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "やめる" }));

    expect(api.updateMyLessonMenu).not.toHaveBeenCalled();
    expect(screen.getByRole("heading", { name: "メニューを登録" })).toBeInTheDocument();
  });

  test("非公開にする: 今の中身のまま is_published を false にして送り、一覧を取り直す", async () => {
    const api = mockApi({ listMyLessonMenus: listOf([LIVE], [{ ...LIVE, is_published: false }]) });
    const user = userEvent.setup();
    render(<LessonMenus api={api} />);

    await user.click(
      await screen.findByRole("button", { name: "「フォーム改善 60 分」を非公開にする" }),
    );

    expect(api.updateMyLessonMenu).toHaveBeenCalledWith(LIVE.lesson_menu_id, {
      sport: "tennis",
      lesson_type: "live_lesson",
      name: "フォーム改善 60 分",
      description: "ストロークを見ます",
      duration_minutes: 60,
      is_published: false,
    });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "「フォーム改善 60 分」を非公開にしました（生徒には見えません）。",
    );
    expect(await screen.findByRole("cell", { name: "非公開" })).toBeInTheDocument();
  });

  test("公開する: 非公開のメニューを公開し直す（非同期の時間は null のまま）", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<LessonMenus api={api} />);

    await user.click(
      await screen.findByRole("button", { name: "「動画でスイング診断」を公開する" }),
    );

    expect(api.updateMyLessonMenu).toHaveBeenCalledWith(ASYNC.lesson_menu_id, {
      sport: "tennis",
      lesson_type: "async_lesson",
      name: "動画でスイング診断",
      description: "",
      duration_minutes: null,
      is_published: true,
    });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "「動画でスイング診断」を公開しました。",
    );
  });

  test("非公開にする API が失敗したらその文言を出す", async () => {
    const api = mockApi({
      updateMyLessonMenu: vi.fn<CoachApi["updateMyLessonMenu"]>(async () => ({
        ok: false,
        message: "対象が見つかりません（ID を確かめてください）。",
      })),
    });
    const user = userEvent.setup();
    render(<LessonMenus api={api} />);

    await user.click(
      await screen.findByRole("button", { name: "「フォーム改善 60 分」を非公開にする" }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "対象が見つかりません（ID を確かめてください）。",
    );
  });

  test("コーチの画面に「レッスンメニュー」のタブがあり、自分のメニューを出す", async () => {
    coachApiMock.mockReturnValue({
      ...mockApi(),
      // 開いたときに出す「対応待ち」と、その上の実績が呼ぶ。
      getMyLessonStats: vi.fn<CoachApi["getMyLessonStats"]>(async () => ({
        ok: true,
        data: EMPTY_LESSON_STATS,
      })),
      listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
        ok: true,
        data: { videos: [] },
      })),
      listMyCoachBookings: vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
        ok: true,
        data: { bookings: [] },
      })),
      listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
        ok: true,
        data: { threads: [] },
      })),
    });
    render(<CoachHome token="mock.coach.x" />);
    await userEvent.click(screen.getByRole("tab", { name: "レッスンメニュー" }));
    const region = screen.getByRole("region", { name: "レッスンメニュー" });

    expect(
      await within(region).findByRole("cell", { name: "フォーム改善 60 分" }),
    ).toBeInTheDocument();
  });
});
