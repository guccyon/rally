import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { CoachHome } from "@/components/coach/CoachHome";
import { PublishSlotForm } from "@/components/coach/PublishSlotForm";
import { addMinutesToLocal, selectableMenus } from "@/components/coach/slotLessonMenu";

import { EMPTY_LESSON_STATS } from "./lessonStatsFixtures";

// 配置のテストでは CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

const COACH_ID = "22222222-2222-4222-8222-222222222222";
// 422 の失敗。画面は `code` で見分ける（文言は API クライアントのものとわざと変え、文言で判定していないことを見る）。
const VALIDATION_FAILED = {
  ok: false,
  code: "validation_failed",
  message: "入力の内容が正しくありません（テスト）。",
} as const;

function menu(id: string, overrides: Partial<Schemas["LessonMenu"]> = {}): Schemas["LessonMenu"] {
  return {
    lesson_menu_id: id,
    coach_id: COACH_ID,
    sport: "tennis",
    lesson_type: "live_lesson",
    name: "フォーム改善",
    description: "",
    duration_minutes: 60,
    is_published: true,
    created_at: "2026-10-30T01:00:00Z",
    ...overrides,
  };
}

// API の並び（作った時刻の古い順）。選べるのはテニスの公開中の同期の 2 件だけ。
const FORM = menu("a0000000-0000-4000-8000-000000000001", { name: "フォーム改善", duration_minutes: 60 });
const SERVE = menu("a0000000-0000-4000-8000-000000000002", { name: "サーブ特訓", duration_minutes: 90 });
const HIDDEN = menu("a0000000-0000-4000-8000-000000000003", { name: "非公開の特訓", is_published: false });
const ASYNC = menu("a0000000-0000-4000-8000-000000000004", {
  name: "動画でスイング診断",
  lesson_type: "async_lesson",
  duration_minutes: null,
});
const SOCCER = menu("a0000000-0000-4000-8000-000000000005", {
  name: "リフティング入門",
  sport: "soccer",
  duration_minutes: 30,
});
const MENUS = [FORM, SERVE, HIDDEN, ASYNC, SOCCER];

function menusOf(...lists: Schemas["LessonMenu"][][]) {
  const fn = vi.fn<CoachApi["listMyLessonMenus"]>();
  for (const lessonMenus of lists) {
    fn.mockResolvedValueOnce({ ok: true, data: { coach_id: COACH_ID, lesson_menus: lessonMenus } });
  }
  return fn;
}

function coachSlot(
  id: string,
  sport: Schemas["Sport"],
  lessonMenu: Schemas["SlotLessonMenu"] | null,
): Schemas["CoachSlot"] {
  const startsAt = new Date(Date.now() + 24 * 60 * 60 * 1000);
  return {
    slot: {
      slot_id: id,
      coach_id: COACH_ID,
      sport,
      starts_at: startsAt.toISOString(),
      ends_at: new Date(startsAt.getTime() + 60 * 60 * 1000).toISOString(),
      status: "open",
      capacity: 1,
      remaining_seats: 1,
      lesson_menu: lessonMenu,
    },
    has_active_booking: false,
  };
}

function slotsOf(...lists: Schemas["CoachSlot"][][]) {
  const fn = vi.fn<CoachApi["listMyCoachSlots"]>();
  for (const slots of lists) fn.mockResolvedValueOnce({ ok: true, data: { slots } });
  return fn;
}

type SlotApi = Pick<CoachApi, "publishSlot" | "listMyCoachSlots" | "listMyLessonMenus" | "withdrawMySlot">;

function mockApi(overrides: Partial<SlotApi> = {}): SlotApi {
  return {
    publishSlot: vi.fn<CoachApi["publishSlot"]>(async (body) => ({
      ok: true,
      data: coachSlot("c0000000-0000-4000-8000-000000000099", body.sport, null).slot,
    })),
    listMyCoachSlots: slotsOf([], []),
    listMyLessonMenus: menusOf(MENUS),
    withdrawMySlot: vi.fn(),
    ...overrides,
  };
}

function optionLabels(select: HTMLElement) {
  return within(select)
    .getAllByRole("option")
    .map((option) => option.textContent);
}

beforeEach(() => {
  coachApiMock.mockReset();
});

describe("枠に結びつけられるメニューと終了の時刻", () => {
  test("公開中・同期・同じスポーツのメニューだけを API の順で返す", () => {
    expect(selectableMenus(MENUS, "tennis")).toEqual([FORM, SERVE]);
    expect(selectableMenus(MENUS, "soccer")).toEqual([SOCCER]);
    expect(selectableMenus(MENUS, "golf")).toEqual([]);
  });

  test("datetime-local の値に分を足し、日をまたげば日付も進める。開始が空なら空", () => {
    expect(addMinutesToLocal("2026-11-01T10:00", 60)).toBe("2026-11-01T11:00");
    expect(addMinutesToLocal("2026-11-01T23:30", 45)).toBe("2026-11-02T00:15");
    expect(addMinutesToLocal("", 60)).toBe("");
  });
});

describe("枠の公開でレッスンメニューを選ぶ", () => {
  test("選択肢は「指定しない」と、枠のスポーツの公開中の同期のメニュー。スポーツを変えると選び直す", async () => {
    const user = userEvent.setup();
    render(<PublishSlotForm api={mockApi()} />);

    const menuSelect = screen.getByLabelText("レッスンメニュー");
    await within(menuSelect).findByRole("option", { name: "フォーム改善（60 分）" });
    expect(optionLabels(menuSelect)).toEqual(["指定しない", "フォーム改善（60 分）", "サーブ特訓（90 分）"]);
    expect(menuSelect).toHaveDisplayValue("指定しない");

    await user.selectOptions(menuSelect, "サーブ特訓（90 分）");
    await user.selectOptions(screen.getByLabelText("スポーツ"), "サッカー");
    expect(optionLabels(menuSelect)).toEqual(["指定しない", "リフティング入門（30 分）"]);
    expect(menuSelect).toHaveDisplayValue("指定しない");

    await user.selectOptions(screen.getByLabelText("スポーツ"), "ゴルフ");
    expect(optionLabels(menuSelect)).toEqual(["指定しない"]);
    expect(menuSelect).toBeDisabled();
    expect(
      screen.getByText("ゴルフの公開中の同期のレッスンメニューはありません。メニューを指定せずに公開できます。"),
    ).toBeInTheDocument();
  });

  test("メニューを選ぶと終了は開始＋メニューの時間になり手で変えられない。lesson_menu_id を付けて送る", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<PublishSlotForm api={api} />);

    const menuSelect = screen.getByLabelText("レッスンメニュー");
    await within(menuSelect).findByRole("option", { name: "サーブ特訓（90 分）" });
    await user.type(screen.getByLabelText("終了"), "2026-11-01T10:30");
    await user.selectOptions(menuSelect, "サーブ特訓（90 分）");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");

    const ends = screen.getByLabelText("終了");
    expect(ends).toHaveValue("2026-11-01T11:30");
    expect(ends).toHaveAttribute("readonly");
    expect(screen.getByText("終了は開始からメニューの時間（90 分）で決まります。")).toBeInTheDocument();
    await user.type(ends, "2026-11-01T12:00");
    expect(ends).toHaveValue("2026-11-01T11:30");

    await user.click(screen.getByRole("button", { name: "枠を公開" }));
    expect(api.publishSlot).toHaveBeenCalledWith({
      sport: "tennis",
      starts_at: new Date("2026-11-01T10:00").toISOString(),
      ends_at: new Date("2026-11-01T11:30").toISOString(),
      capacity: 1,
      lesson_menu_id: SERVE.lesson_menu_id,
    });
    expect(await screen.findByRole("status")).toHaveTextContent("枠を公開しました。");
  });

  test("「指定しない」に戻すと終了を手で入れられ、lesson_menu_id を送らない", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<PublishSlotForm api={api} />);

    const menuSelect = screen.getByLabelText("レッスンメニュー");
    await within(menuSelect).findByRole("option", { name: "フォーム改善（60 分）" });
    await user.selectOptions(menuSelect, "フォーム改善（60 分）");
    await user.selectOptions(menuSelect, "指定しない");
    expect(screen.getByLabelText("終了")).not.toHaveAttribute("readonly");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T10:45");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(api.publishSlot).toHaveBeenCalledWith({
      sport: "tennis",
      starts_at: new Date("2026-11-01T10:00").toISOString(),
      ends_at: new Date("2026-11-01T10:45").toISOString(),
      capacity: 1,
    });
  });

  test("メニューを選んで開始が空なら、API を呼ばずに開始を入れるよう知らせる", async () => {
    const api = mockApi();
    const user = userEvent.setup();
    render(<PublishSlotForm api={api} />);

    const menuSelect = screen.getByLabelText("レッスンメニュー");
    await within(menuSelect).findByRole("option", { name: "フォーム改善（60 分）" });
    await user.selectOptions(menuSelect, "フォーム改善（60 分）");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(api.publishSlot).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("開始の日時を入力してください。");
  });

  test("メニューを付けてサーバーが断った（422）ら、変わった可能性を伝えてメニューを読み直し、選べなくなったものは外す", async () => {
    const api = mockApi({
      publishSlot: vi.fn<CoachApi["publishSlot"]>(async () => VALIDATION_FAILED),
      // 読み込んだ後に「サーブ特訓」が非公開にされた。
      listMyLessonMenus: menusOf(MENUS, [FORM, { ...SERVE, is_published: false }]),
    });
    const user = userEvent.setup();
    render(<PublishSlotForm api={api} />);

    const menuSelect = screen.getByLabelText("レッスンメニュー");
    await within(menuSelect).findByRole("option", { name: "サーブ特訓（90 分）" });
    await user.selectOptions(menuSelect, "サーブ特訓（90 分）");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "選んだレッスンメニューは、この枠に結びつけられませんでした。メニューが非公開になったか、スポーツや時間が変わった可能性があります。",
    );
    expect(api.listMyLessonMenus).toHaveBeenCalledTimes(2);
    expect(optionLabels(menuSelect)).toEqual(["指定しない", "フォーム改善（60 分）"]);
    expect(menuSelect).toHaveDisplayValue("指定しない");
    expect(api.listMyCoachSlots).toHaveBeenCalledTimes(1);
  });

  test("メニューを付けても 422 以外の失敗（枠の重なり）なら、その文言だけを出してメニューは読み直さない", async () => {
    const api = mockApi({
      publishSlot: vi.fn<CoachApi["publishSlot"]>(async () => ({
        ok: false,
        code: "slot_overlaps",
        message: "公開済みの枠と時間が重なっています。",
      })),
    });
    const user = userEvent.setup();
    render(<PublishSlotForm api={api} />);

    const menuSelect = screen.getByLabelText("レッスンメニュー");
    await within(menuSelect).findByRole("option", { name: "フォーム改善（60 分）" });
    await user.selectOptions(menuSelect, "フォーム改善（60 分）");
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/^公開済みの枠と時間が重なっています。$/);
    expect(api.listMyLessonMenus).toHaveBeenCalledTimes(1);
    expect(menuSelect).toHaveDisplayValue("フォーム改善（60 分）");
  });

  test("メニューを付けずにサーバーが断ったら、今までどおりその文言だけを出す", async () => {
    const api = mockApi({
      publishSlot: vi.fn<CoachApi["publishSlot"]>(async () => VALIDATION_FAILED),
    });
    const user = userEvent.setup();
    render(<PublishSlotForm api={api} />);

    await within(screen.getByLabelText("レッスンメニュー")).findByRole("option", {
      name: "フォーム改善（60 分）",
    });
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/^入力の内容が正しくありません（テスト）。$/);
    expect(api.listMyLessonMenus).toHaveBeenCalledTimes(1);
  });

  test("メニューの一覧を読めなくても、メニューを指定せずに公開できる", async () => {
    const api = mockApi({
      listMyLessonMenus: vi.fn<CoachApi["listMyLessonMenus"]>(async () => ({
        ok: false,
        message: "サーバーに接続できません。",
      })),
    });
    const user = userEvent.setup();
    render(<PublishSlotForm api={api} />);

    expect(
      await screen.findByText(
        "レッスンメニューを読み込めませんでした（サーバーに接続できません。）。メニューを指定せずに公開できます。",
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("レッスンメニュー")).toBeDisabled();
    await user.type(screen.getByLabelText("開始"), "2026-11-01T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-01T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(api.publishSlot).toHaveBeenCalledWith({
      sport: "tennis",
      starts_at: new Date("2026-11-01T10:00").toISOString(),
      ends_at: new Date("2026-11-01T11:00").toISOString(),
      capacity: 1,
    });
  });

  test("自分の枠の一覧に、結びついたメニューの名前を出す（ない枠は「—」）", async () => {
    const api = mockApi({
      listMyCoachSlots: slotsOf([
        coachSlot("c0000000-0000-4000-8000-000000000001", "tennis", {
          lesson_menu_id: SERVE.lesson_menu_id,
          name: "サーブ特訓",
          duration_minutes: 90,
        }),
        coachSlot("c0000000-0000-4000-8000-000000000002", "soccer", null),
      ]),
    });
    render(<PublishSlotForm api={api} />);

    const upcoming = await screen.findByRole("table", { name: "これからの枠" });
    expect(within(upcoming).getByRole("columnheader", { name: "メニュー" })).toBeInTheDocument();
    expect(within(upcoming).getByRole("row", { name: /テニス/ })).toHaveTextContent("サーブ特訓");
    const soccer = within(upcoming).getByRole("row", { name: /サッカー/ });
    expect(within(soccer).getAllByRole("cell")[1]).toHaveTextContent(/^—$/);
  });
});

describe("コーチの画面の「予約枠」", () => {
  test("予約枠のタブでメニューを選んで枠を公開し、一覧にメニューの名前が出る", async () => {
    const published = coachSlot("c0000000-0000-4000-8000-000000000003", "tennis", {
      lesson_menu_id: FORM.lesson_menu_id,
      name: "フォーム改善",
      duration_minutes: 60,
    });
    const api = mockApi({ listMyCoachSlots: slotsOf([], [published]) });
    coachApiMock.mockReturnValue({
      ...api,
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
    const user = userEvent.setup();
    render(<CoachHome token="mock.coach.x" />);
    await user.click(screen.getByRole("tab", { name: "予約枠" }));
    const region = screen.getByRole("region", { name: "予約枠" });

    const menuSelect = within(region).getByLabelText("レッスンメニュー");
    await within(menuSelect).findByRole("option", { name: "フォーム改善（60 分）" });
    await user.selectOptions(menuSelect, "フォーム改善（60 分）");
    await user.type(within(region).getByLabelText("開始"), "2026-11-01T10:00");
    await user.click(within(region).getByRole("button", { name: "枠を公開" }));

    expect(api.publishSlot).toHaveBeenCalledWith(
      expect.objectContaining({
        ends_at: new Date("2026-11-01T11:00").toISOString(),
        lesson_menu_id: FORM.lesson_menu_id,
      }),
    );
    const upcoming = await within(region).findByRole("table", { name: "これからの枠" });
    expect(within(upcoming).getByRole("row", { name: /テニス/ })).toHaveTextContent("フォーム改善");
  });
});
