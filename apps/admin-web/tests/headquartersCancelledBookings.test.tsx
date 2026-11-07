import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { CancelledBookingList } from "@/components/headquarters/CancelledBookingList";
import {
  CancelledBookingsHome,
  cancelledBookingCancellerOf,
  cancelledBookingSportOf,
  cancelledBookingsPath,
} from "@/components/headquarters/CancelledBookingsHome";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { SPORTS } from "@/components/headquarters/PendingVideoReviewList";
import { SPORT_LABELS, formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 配置のテストでは CancelledBookingsHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { headquartersApiMock } = vi.hoisted(() => ({ headquartersApiMock: vi.fn() }));
vi.mock("@/api/headquarters", () => ({ headquartersApi: headquartersApiMock }));

beforeEach(() => {
  replace.mockReset();
  headquartersApiMock.mockReset();
});

const SUZUKI = "22222222-2222-4222-8222-000000000001";
const TAKAHASHI = "22222222-2222-4222-8222-000000000002";

const COACHES: Schemas["Coach"][] = [
  { coach_id: SUZUKI, display_name: "鈴木 コーチ", acceptance_status: "accepting", recent_lesson_count: 3 },
  { coach_id: TAKAHASHI, display_name: "高橋 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
];

const FOREHAND = "aaaaaaaa-aaaa-4aaa-8aaa-000000000001";
const SWING = "aaaaaaaa-aaaa-4aaa-8aaa-000000000002";

function lessonMenu(
  lessonMenuId: string,
  name: string,
  coachId: string,
  coachDisplayName?: string,
): Schemas["CoachLessonMenu"] {
  return {
    lesson_menu_id: lessonMenuId,
    name,
    sport: "tennis",
    lesson_type: "live_lesson",
    duration_minutes: 60,
    is_published: true,
    coach_id: coachId,
    ...(coachDisplayName === undefined ? {} : { coach_display_name: coachDisplayName }),
    created_at: "2026-10-01T00:00:00Z",
    recent_booking_count: 1,
  };
}

const LESSON_MENUS: Schemas["CoachLessonMenu"][] = [
  lessonMenu(FOREHAND, "フォアハンドの基礎", SUZUKI, "鈴木 コーチ"),
  lessonMenu(SWING, "スイングの見直し", TAKAHASHI),
];

// API が返す並び（キャンセルした時刻の新しい順）。画面はこの順と値のまま出す。
const BOOKINGS: Schemas["CancelledBooking"][] = [
  {
    booking_id: "44444444-4444-4444-8444-000000000001",
    student_id: "11111111-1111-4111-8111-000000000001",
    student_display_name: "佐藤 太郎",
    coach_id: "22222222-2222-4222-8222-000000000001",
    coach_display_name: "高橋 コーチ",
    starts_at: "2026-10-28T01:00:00Z",
    cancelled_at: "2026-10-25T20:10:00Z",
    cancelled_by: "student",
  },
  {
    booking_id: "44444444-4444-4444-8444-000000000002",
    student_id: "11111111-1111-4111-8111-000000000002",
    coach_id: "22222222-2222-4222-8222-000000000002",
    starts_at: "2026-10-21T09:00:00Z",
    cancelled_at: "2026-10-20T01:59:00Z",
    cancelled_by: null,
  },
];

type Response = Awaited<ReturnType<HeadquartersApi["listCancelledBookings"]>>;

function listOf(bookings: Schemas["CancelledBooking"][], from = "2026-10-20", to = "2026-10-26") {
  return vi.fn<HeadquartersApi["listCancelledBookings"]>(async () => ({
    ok: true,
    data: { from, to, bookings },
  }));
}

function coachesOf(coaches: Schemas["Coach"][] = COACHES) {
  return vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches } }));
}

function lessonMenusOf(lessonMenus: Schemas["CoachLessonMenu"][] = LESSON_MENUS) {
  return vi.fn<HeadquartersApi["listLessonMenus"]>(async () => ({ ok: true, data: { lesson_menus: lessonMenus } }));
}

/** 絞り込みの選択肢（コーチ・レッスンメニュー）がどちらも読める API。 */
function choicesOf() {
  return { listCoaches: coachesOf(), listLessonMenus: lessonMenusOf() };
}

function cellsOf(row: HTMLElement): (string | null)[] {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

async function changePeriod(user: ReturnType<typeof userEvent.setup>, from: string, to: string) {
  await user.clear(screen.getByLabelText("開始日"));
  await user.type(screen.getByLabelText("開始日"), from);
  await user.clear(screen.getByLabelText("終了日"));
  await user.type(screen.getByLabelText("終了日"), to);
}

describe("本部のキャンセルされた予約", () => {
  test("読み込み中を出し、開いたときに期間を付けずに読み、各行にキャンセルした日時・枠の開始・コーチ・生徒・キャンセルした人を API の順と値のまま出す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);

    expect(screen.getByText("キャンセルされた予約を読み込んでいます…")).toBeInTheDocument();
    // 期間は API の既定を読むまで変えられない。キャンセルした人は固定の選択肢なので読み込み中も選べる。
    expect(screen.getByText("期間を読み込んでいます…")).toBeInTheDocument();
    expect(screen.getByLabelText("開始日")).toBeDisabled();
    expect(screen.getByRole("progressbar")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "キャンセルした人" })).toBeEnabled();
    const table = await screen.findByRole("table", { name: "キャンセルされた予約の一覧" });
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
    expect(listCancelledBookings).toHaveBeenCalledWith({});
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map(cellsOf)).toEqual([
      [
        formatDateTime("2026-10-25T20:10:00Z"),
        formatDateTime("2026-10-28T01:00:00Z"),
        "高橋 コーチ",
        "佐藤 太郎",
        "生徒",
      ],
      [
        formatDateTime("2026-10-20T01:59:00Z"),
        formatDateTime("2026-10-21T09:00:00Z"),
        "（名前を表示できません）",
        "（名前を表示できません）",
        "記録なし",
      ],
    ]);
    expect(screen.queryByText("キャンセルされた予約を読み込んでいます…")).not.toBeInTheDocument();
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.getByLabelText("開始日")).toBeEnabled();
  });

  test("API の既定で実際に使った期間を件数とともに出し、期間の欄にも入れる", async () => {
    render(<CancelledBookingList api={{ listCancelledBookings: listOf(BOOKINGS), ...choicesOf() }} />);

    expect(await screen.findByText("2026年10月20日（火） 〜 2026年10月26日（月）・2 件")).toBeInTheDocument();
    expect(screen.getByLabelText("開始日")).toHaveValue("2026-10-20");
    expect(screen.getByLabelText("終了日")).toHaveValue("2026-10-26");
  });

  test("期間を選んで「この条件で見る」で API の from / to に渡して読み直す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-10-01", "2026-10-31");
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(listCancelledBookings).toHaveBeenLastCalledWith({ from: "2026-10-01", to: "2026-10-31" });
  });

  test("終了日が開始日より前・31 日を超える期間は送らずに理由を出す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-10-20", "2026-10-19");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "終了日は開始日と同じか、それより後の日付にしてください。",
    );

    await changePeriod(user, "2026-09-01", "2026-10-02");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("期間は 31 日までにしてください。");
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
  });

  test("API の 422 は、期間・コーチ・レッスンメニュー・スポーツ・キャンセルした人のどこを直すかを添えた理由を出し、一覧は出さない", async () => {
    const listCancelledBookings = vi.fn<HeadquartersApi["listCancelledBookings"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力の内容が正しくありません。",
    }));
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "期間・コーチ・レッスンメニュー・スポーツ・キャンセルした人の指定が正しくありません。期間は 1〜31 日で、終了日は開始日と同じかそれより後の日付にしてください。コーチ・レッスンメニュー・スポーツ・キャンセルした人は一覧から選び直してください。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("読み直しのあいだは「この条件で見る」を loading にして押せなくし（二重押しで送らない）、一覧を「読み込んでいます…」にする", async () => {
    let resolveSlow: (value: Response) => void = () => {};
    const listCancelledBookings = vi
      .fn<HeadquartersApi["listCancelledBookings"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } })
      .mockImplementationOnce(() => new Promise((resolve) => (resolveSlow = resolve)));
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-10-25", "2026-10-26");
    const button = screen.getByRole("button", { name: "この条件で見る" });
    await user.click(button);
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    expect(within(button).getByTestId("spinner")).toBeInTheDocument();
    expect(screen.getByText("キャンセルされた予約を読み込んでいます…")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    await user.click(button);
    expect(listCancelledBookings).toHaveBeenCalledTimes(2);

    resolveSlow({ ok: true, data: { from: "2026-10-25", to: "2026-10-26", bookings: [BOOKINGS[0]!] } });
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.getByRole("row", { name: /佐藤 太郎/ })).toBeInTheDocument();
    expect(button).toBeEnabled();
  });

  test("コーチの選択肢はコーチの一覧の順で、期間とコーチを選んで「この条件で見る」で API の from / to / coach_id に渡して読み直す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    const onFilterChange = vi.fn();
    const user = userEvent.setup();
    render(
      <CancelledBookingList
        api={{ listCancelledBookings, ...choicesOf() }}
        onFilterChange={onFilterChange}
      />,
    );
    await screen.findByRole("table");

    const select = screen.getByRole("combobox", { name: "コーチ" });
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "鈴木 コーチ",
      "高橋 コーチ",
    ]);
    await user.selectOptions(select, "高橋 コーチ");
    await changePeriod(user, "2026-10-01", "2026-10-31");
    // 選んだだけでは読み直さない。
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(listCancelledBookings).toHaveBeenLastCalledWith({
      from: "2026-10-01",
      to: "2026-10-31",
      coach_id: TAKAHASHI,
    });
    expect(onFilterChange).toHaveBeenCalledWith({
      period: { from: "2026-10-01", to: "2026-10-31" },
      coachId: TAKAHASHI,
      lessonMenuId: "",
      sport: "",
      cancelledBy: "",
    });

    await user.selectOptions(select, "すべて");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(listCancelledBookings).toHaveBeenLastCalledWith({ from: "2026-10-01", to: "2026-10-31" });
  });

  test("渡されたコーチ（URL のクエリ）で開き、期間は付けずに読んでコーチの欄にも入れる", async () => {
    const listCancelledBookings = listOf([BOOKINGS[0]!]);
    render(
      <CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} initialCoachId={SUZUKI} />,
    );

    await screen.findByRole("table");
    expect(listCancelledBookings).toHaveBeenCalledWith({ coach_id: SUZUKI });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toHaveValue(SUZUKI));
    expect(screen.getByLabelText("開始日")).toHaveValue("2026-10-20");
  });

  test("コーチで絞って 0 件なら「この期間にこのコーチの枠でキャンセルされた予約はありません」と出す", async () => {
    render(
      <CancelledBookingList
        api={{ listCancelledBookings: listOf([]), ...choicesOf() }}
        initialCoachId={TAKAHASHI}
      />,
    );

    expect(await screen.findByText("この期間にこのコーチの枠でキャンセルされた予約はありません。")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("コーチの一覧が読めなくても期間だけで一覧を出し、コーチの絞り込みだけを使えなくする", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<CancelledBookingList api={{ listCancelledBookings: listOf(BOOKINGS), listCoaches, listLessonMenus: lessonMenusOf() }} />);

    expect(await screen.findByRole("table", { name: "キャンセルされた予約の一覧" })).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("コーチの一覧を読めませんでした。サーバーに接続できません。");
    expect(screen.getByRole("combobox", { name: "コーチ" })).toBeDisabled();
  });

  test("レッスンメニューの選択肢はレッスンメニューの一覧（絞り込みなし）の順でコーチの名前を添え、選んで「この条件で見る」で API の lesson_menu_id に渡して読み直し、絞っているメニューの名前を出す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    const listLessonMenus = lessonMenusOf();
    const onFilterChange = vi.fn();
    const user = userEvent.setup();
    render(
      <CancelledBookingList
        api={{ listCancelledBookings, listCoaches: coachesOf(), listLessonMenus }}
        onFilterChange={onFilterChange}
      />,
    );
    await screen.findByRole("table");
    expect(listLessonMenus).toHaveBeenCalledWith({});

    const select = screen.getByRole("combobox", { name: "レッスンメニュー" });
    await waitFor(() => expect(select).toBeEnabled());
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "フォアハンドの基礎（鈴木 コーチ）",
      "スイングの見直し",
    ]);
    expect(screen.queryByText(/^レッスンメニューで絞っています:/)).not.toBeInTheDocument();

    await user.selectOptions(select, "フォアハンドの基礎（鈴木 コーチ）");
    // 選んだだけでは読み直さない。
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(listCancelledBookings).toHaveBeenLastCalledWith({
      from: "2026-10-20",
      to: "2026-10-26",
      lesson_menu_id: FOREHAND,
    });
    expect(onFilterChange).toHaveBeenCalledWith({
      period: { from: "2026-10-20", to: "2026-10-26" },
      coachId: "",
      lessonMenuId: FOREHAND,
      sport: "",
      cancelledBy: "",
    });
    expect(await screen.findByText(/^レッスンメニューで絞っています:/)).toHaveTextContent(
      "レッスンメニューで絞っています: フォアハンドの基礎",
    );

    await user.selectOptions(select, "すべて");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(listCancelledBookings).toHaveBeenLastCalledWith({ from: "2026-10-20", to: "2026-10-26" });
    expect(screen.queryByText(/^レッスンメニューで絞っています:/)).not.toBeInTheDocument();
  });

  test("期間・コーチ・レッスンメニューを組み合わせて API の from / to / coach_id / lesson_menu_id に渡し、0 件ならそのコーチの枠のそのメニューのことだと言う", async () => {
    const listCancelledBookings = vi
      .fn<HeadquartersApi["listCancelledBookings"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } })
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-01", to: "2026-10-31", bookings: [] } });
    const user = userEvent.setup();
    render(
      <CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />,
    );
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByRole("combobox", { name: "レッスンメニュー" })).toBeEnabled());

    await changePeriod(user, "2026-10-01", "2026-10-31");
    await user.selectOptions(screen.getByRole("combobox", { name: "コーチ" }), "高橋 コーチ");
    await user.selectOptions(screen.getByRole("combobox", { name: "レッスンメニュー" }), "スイングの見直し");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(listCancelledBookings).toHaveBeenLastCalledWith({
      from: "2026-10-01",
      to: "2026-10-31",
      coach_id: TAKAHASHI,
      lesson_menu_id: SWING,
    });
    expect(
      await screen.findByText("この期間にこのコーチの枠のこのレッスンメニューでキャンセルされた予約はありません。"),
    ).toBeInTheDocument();
  });

  test("渡されたレッスンメニュー（URL のクエリ）で開き、期間は付けずに読んでメニューの欄と絞っているメニューの名前に出す。0 件ならそのメニューのことだと言う", async () => {
    const listCancelledBookings = listOf([]);
    render(
      <CancelledBookingList
        api={{ listCancelledBookings, ...choicesOf() }}
        initialLessonMenuId={SWING}
      />,
    );

    expect(
      await screen.findByText("この期間にこのレッスンメニューでキャンセルされた予約はありません。"),
    ).toBeInTheDocument();
    expect(listCancelledBookings).toHaveBeenCalledWith({ lesson_menu_id: SWING });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "レッスンメニュー" })).toHaveValue(SWING));
    expect(await screen.findByText(/^レッスンメニューで絞っています:/)).toHaveTextContent(
      "レッスンメニューで絞っています: スイングの見直し",
    );
  });

  test("レッスンメニューの一覧が読めなくても一覧を出し、レッスンメニューの絞り込みだけを使えなくする", async () => {
    const listLessonMenus = vi.fn<HeadquartersApi["listLessonMenus"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(
      <CancelledBookingList
        api={{ listCancelledBookings: listOf(BOOKINGS), listCoaches: coachesOf(), listLessonMenus }}
      />,
    );

    expect(await screen.findByRole("table", { name: "キャンセルされた予約の一覧" })).toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "レッスンメニューの一覧を読めませんでした。サーバーに接続できません。",
    );
    expect(screen.getByRole("combobox", { name: "レッスンメニュー" })).toBeDisabled();
    expect(screen.getByRole("combobox", { name: "コーチ" })).toBeEnabled();
  });

  test("スポーツの選択肢は「すべて」と各スポーツで、選んで「この条件で見る」で API の sport に渡して読み直し、件数の行に選んだスポーツを出す", async () => {
    const listCancelledBookings = vi
      .fn<HeadquartersApi["listCancelledBookings"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } })
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: [BOOKINGS[0]!] } })
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } });
    const onFilterChange = vi.fn();
    const user = userEvent.setup();
    render(
      <CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} onFilterChange={onFilterChange} />,
    );
    await screen.findByRole("table");

    const select = screen.getByRole("combobox", { name: "スポーツ" });
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      ...SPORTS.map((sport) => SPORT_LABELS[sport]),
    ]);
    await user.selectOptions(select, "テニス");
    // 選んだだけでは読み直さない（期間・コーチ・レッスンメニューと同じく「この条件で見る」で読み直す）。
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(listCancelledBookings).toHaveBeenLastCalledWith({ from: "2026-10-20", to: "2026-10-26", sport: "tennis" });
    expect(onFilterChange).toHaveBeenCalledWith({
      period: { from: "2026-10-20", to: "2026-10-26" },
      coachId: "",
      lessonMenuId: "",
      sport: "tennis",
      cancelledBy: "",
    });
    expect(await screen.findByText("2026年10月20日（火） 〜 2026年10月26日（月）・テニス 1 件")).toBeInTheDocument();

    await user.selectOptions(select, "すべて");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(listCancelledBookings).toHaveBeenLastCalledWith({ from: "2026-10-20", to: "2026-10-26" });
    expect(await screen.findByText("2026年10月20日（火） 〜 2026年10月26日（月）・2 件")).toBeInTheDocument();
  });

  test("期間・コーチ・レッスンメニュー・スポーツを組み合わせて API の from / to / coach_id / lesson_menu_id / sport に渡し、0 件ならその競技のそのコーチの枠のそのメニューのことだと言う", async () => {
    const listCancelledBookings = vi
      .fn<HeadquartersApi["listCancelledBookings"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } })
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-01", to: "2026-10-31", bookings: [] } });
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByRole("combobox", { name: "レッスンメニュー" })).toBeEnabled());

    await changePeriod(user, "2026-10-01", "2026-10-31");
    await user.selectOptions(screen.getByRole("combobox", { name: "コーチ" }), "高橋 コーチ");
    await user.selectOptions(screen.getByRole("combobox", { name: "レッスンメニュー" }), "スイングの見直し");
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "ゴルフ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(listCancelledBookings).toHaveBeenLastCalledWith({
      from: "2026-10-01",
      to: "2026-10-31",
      coach_id: TAKAHASHI,
      lesson_menu_id: SWING,
      sport: "golf",
    });
    expect(
      await screen.findByText("この期間にゴルフのこのコーチの枠のこのレッスンメニューでキャンセルされた予約はありません。"),
    ).toBeInTheDocument();
    // 0 件のときは件数を出さない
    expect(screen.getByText("2026年10月1日（木） 〜 2026年10月31日（土）・ゴルフ")).toBeInTheDocument();
  });

  test("渡されたスポーツ（URL のクエリ）で開き、期間は付けずに読んでスポーツの欄にも入れる。0 件ならその競技のことだと言う", async () => {
    const listCancelledBookings = listOf([]);
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} initialSport="swimming" />);

    expect(await screen.findByText("この期間に水泳でキャンセルされた予約はありません。")).toBeInTheDocument();
    expect(listCancelledBookings).toHaveBeenCalledWith({ sport: "swimming" });
    expect(screen.getByRole("combobox", { name: "スポーツ" })).toHaveValue("swimming");
    expect(screen.getByLabelText("開始日")).toHaveValue("2026-10-20");
  });

  test("キャンセルした人の選択肢は「すべて」・生徒・コーチ・本部で、選んで「この条件で見る」で API の cancelled_by に渡して読み直し、件数の行に選んだキャンセルした人を出す", async () => {
    const byHeadquarters: Schemas["CancelledBooking"] = { ...BOOKINGS[0]!, cancelled_by: "headquarters" };
    const listCancelledBookings = vi
      .fn<HeadquartersApi["listCancelledBookings"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } })
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: [byHeadquarters] } })
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } });
    const onFilterChange = vi.fn();
    const user = userEvent.setup();
    render(
      <CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} onFilterChange={onFilterChange} />,
    );
    await screen.findByRole("table");

    const select = screen.getByRole("combobox", { name: "キャンセルした人" });
    expect(select).toHaveValue("");
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "生徒",
      "コーチ",
      "本部",
    ]);
    await user.selectOptions(select, "本部");
    // 選んだだけでは読み直さず、件数の行にも出さない（ほかの絞り込みと同じく「この条件で見る」で読み直す）。
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(/キャンセルした人: /)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(listCancelledBookings).toHaveBeenLastCalledWith({
      from: "2026-10-20",
      to: "2026-10-26",
      cancelled_by: "headquarters",
    });
    expect(onFilterChange).toHaveBeenCalledWith({
      period: { from: "2026-10-20", to: "2026-10-26" },
      coachId: "",
      lessonMenuId: "",
      sport: "",
      cancelledBy: "headquarters",
    });
    expect(
      await screen.findByText("2026年10月20日（火） 〜 2026年10月26日（月）・キャンセルした人: 本部・1 件"),
    ).toHaveAttribute("role", "status");
    // 本部で絞っても「キャンセルした人」の列は消さない
    const table = screen.getByRole("table", { name: "キャンセルされた予約の一覧" });
    expect(within(table).getByRole("columnheader", { name: "キャンセルした人" })).toBeInTheDocument();
    expect(cellsOf(within(table).getAllByRole("row")[1]!).at(-1)).toBe("本部");

    // 「すべて」に戻せば cancelled_by を付けずに読み直す
    await user.selectOptions(select, "すべて");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(listCancelledBookings).toHaveBeenLastCalledWith({ from: "2026-10-20", to: "2026-10-26" });
    expect(await screen.findByText("2026年10月20日（火） 〜 2026年10月26日（月）・2 件")).toBeInTheDocument();
  });

  test("キャンセルした人とスポーツ・コーチを組み合わせて送り、件数の行にはキャンセルした人とスポーツの件数を並べる", async () => {
    const listCancelledBookings = vi
      .fn<HeadquartersApi["listCancelledBookings"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } })
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-01", to: "2026-10-31", bookings: [BOOKINGS[0]!] } });
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toBeEnabled());

    await changePeriod(user, "2026-10-01", "2026-10-31");
    await user.selectOptions(screen.getByRole("combobox", { name: "コーチ" }), "高橋 コーチ");
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await user.selectOptions(screen.getByRole("combobox", { name: "キャンセルした人" }), "生徒");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(listCancelledBookings).toHaveBeenLastCalledWith({
      from: "2026-10-01",
      to: "2026-10-31",
      coach_id: TAKAHASHI,
      sport: "tennis",
      cancelled_by: "student",
    });
    expect(
      await screen.findByText("2026年10月1日（木） 〜 2026年10月31日（土）・キャンセルした人: 生徒・テニス 1 件"),
    ).toBeInTheDocument();
  });

  test("渡されたキャンセルした人（URL のクエリ）で開いて読み、0 件なら件数を出さずに、その人がキャンセルした予約がないことと条件の変え方を出す", async () => {
    const listCancelledBookings = listOf([]);
    render(
      <CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} initialCancelledBy="headquarters" />,
    );

    expect(
      await screen.findByText(
        "この期間に本部がキャンセルした予約はありません。キャンセルした人を「すべて」にするか、期間を変えて見てください。",
      ),
    ).toBeInTheDocument();
    expect(listCancelledBookings).toHaveBeenCalledWith({ cancelled_by: "headquarters" });
    expect(screen.getByRole("combobox", { name: "キャンセルした人" })).toHaveValue("headquarters");
    expect(screen.getByText("2026年10月20日（火） 〜 2026年10月26日（月）・キャンセルした人: 本部")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    // 0 件の表示にボタンは置かない（画面の primary は「この条件で見る」の 1 つ）
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

  test("コーチとキャンセルした人で絞って 0 件なら、このコーチの枠でその人がキャンセルした予約はないと言う", async () => {
    render(
      <CancelledBookingList
        api={{ listCancelledBookings: listOf([]), ...choicesOf() }}
        initialCoachId={SUZUKI}
        initialCancelledBy="coach"
      />,
    );

    expect(
      await screen.findByText(
        "この期間にこのコーチの枠でコーチがキャンセルした予約はありません。キャンセルした人を「すべて」にするか、期間を変えて見てください。",
      ),
    ).toBeInTheDocument();
  });

  test("期間が正しくなければ送らず、選んだキャンセルした人は消さない", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);
    await screen.findByRole("table");

    await user.selectOptions(screen.getByRole("combobox", { name: "キャンセルした人" }), "本部");
    await changePeriod(user, "2026-10-20", "2026-10-19");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(screen.getByRole("alert")).toHaveTextContent(
      "終了日が開始日より前です。終了日は開始日と同じか、それより後の日付にしてください。",
    );
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("combobox", { name: "キャンセルした人" })).toHaveValue("headquarters");
  });

  test("0 件なら「この期間にキャンセルされた予約はありません。」と出し、件数の行に件数を出さず、表は出さない", async () => {
    render(<CancelledBookingList api={{ listCancelledBookings: listOf([]), ...choicesOf() }} />);

    expect(await screen.findByText("この期間にキャンセルされた予約はありません。")).toBeInTheDocument();
    expect(screen.getByText("2026年10月20日（火） 〜 2026年10月26日（月）")).toBeInTheDocument();
    expect(screen.queryByText(/0 件/)).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listCancelledBookings = vi.fn<HeadquartersApi["listCancelledBookings"]>(async () => ({
      ok: false,
      code: "forbidden",
      message: "権限がないためアクセスできません。",
    }));
    render(<CancelledBookingList api={{ listCancelledBookings, ...choicesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("この期間にキャンセルされた予約はありません。")).not.toBeInTheDocument();
    expect(screen.queryByText("キャンセルされた予約を読み込んでいます…")).not.toBeInTheDocument();
  });
});

describe("絞り込みの URL", () => {
  test("コーチ・レッスンメニュー・スポーツを契約のクエリの名前で URL に残し、「すべて」のコーチ・レッスンメニュー・スポーツと期間は出さない", () => {
    expect(cancelledBookingsPath({ coachId: "", lessonMenuId: "", sport: "" })).toBe("/headquarters/cancelled-bookings");
    expect(cancelledBookingsPath({ coachId: TAKAHASHI, lessonMenuId: "", sport: "" })).toBe(
      `/headquarters/cancelled-bookings?coach_id=${TAKAHASHI}`,
    );
    expect(cancelledBookingsPath({ coachId: "", lessonMenuId: FOREHAND, sport: "" })).toBe(
      `/headquarters/cancelled-bookings?lesson_menu_id=${FOREHAND}`,
    );
    expect(cancelledBookingsPath({ coachId: "", lessonMenuId: "", sport: "tennis" })).toBe(
      "/headquarters/cancelled-bookings?sport=tennis",
    );
    expect(cancelledBookingsPath({ coachId: TAKAHASHI, lessonMenuId: FOREHAND, sport: "tennis" })).toBe(
      `/headquarters/cancelled-bookings?coach_id=${TAKAHASHI}&lesson_menu_id=${FOREHAND}&sport=tennis`,
    );
  });

  test("キャンセルした人を契約のクエリの名前 cancelled_by で URL に残し、「すべて」なら出さない", () => {
    expect(cancelledBookingsPath({ coachId: "", lessonMenuId: "", sport: "", cancelledBy: "headquarters" })).toBe(
      "/headquarters/cancelled-bookings?cancelled_by=headquarters",
    );
    expect(cancelledBookingsPath({ coachId: TAKAHASHI, lessonMenuId: "", sport: "tennis", cancelledBy: "student" })).toBe(
      `/headquarters/cancelled-bookings?coach_id=${TAKAHASHI}&sport=tennis&cancelled_by=student`,
    );
    expect(cancelledBookingsPath({ coachId: "", lessonMenuId: "", sport: "", cancelledBy: "" })).toBe(
      "/headquarters/cancelled-bookings",
    );
  });

  test("URL のクエリのキャンセルした人は BookingCanceller の値だけを使い、ないかそれ以外の値なら「すべて」で開く", () => {
    expect(cancelledBookingCancellerOf("student")).toBe("student");
    expect(cancelledBookingCancellerOf("coach")).toBe("coach");
    expect(cancelledBookingCancellerOf("headquarters")).toBe("headquarters");
    expect(cancelledBookingCancellerOf(null)).toBe("");
    expect(cancelledBookingCancellerOf("")).toBe("");
    expect(cancelledBookingCancellerOf("Headquarters")).toBe("");
    expect(cancelledBookingCancellerOf("none")).toBe("");
  });

  test("URL のクエリのスポーツは Sport の値だけを使い、ないか Sport の値でなければ「すべて」で開く", () => {
    expect(cancelledBookingSportOf("golf")).toBe("golf");
    expect(cancelledBookingSportOf(null)).toBe("");
    expect(cancelledBookingSportOf("cricket")).toBe("");
  });
});

describe("画面への配置", () => {
  test("本部の画面からキャンセルされた予約の画面へ移れる", () => {
    render(<HeadquartersHome />);

    const region = screen.getByRole("region", { name: "キャンセルされた予約" });
    expect(within(region).getByRole("link", { name: "キャンセルされた予約を見る" })).toHaveAttribute(
      "href",
      "/headquarters/cancelled-bookings",
    );
  });

  test("キャンセルされた予約の画面は開いたときに一覧を読み、本部の画面へ戻れる", async () => {
    headquartersApiMock.mockReturnValue({
      listCancelledBookings: listOf(BOOKINGS),
      listCoaches: coachesOf(),
      listLessonMenus: lessonMenusOf(),
    });
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <CancelledBookingsHome token={session.token} />}
      </RoleGate>,
    );

    const region = screen.getByRole("region", { name: "キャンセルされた予約" });
    expect(
      await within(region).findByRole("table", { name: "キャンセルされた予約の一覧" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "本部の画面へ戻る" })).toHaveAttribute("href", "/headquarters");
    expect(replace).not.toHaveBeenCalled();
  });

  test("キャンセルされた予約の画面で「この条件で見る」を押すと、選んだコーチを URL のクエリに残す（履歴は積まない）", async () => {
    headquartersApiMock.mockReturnValue({
      listCancelledBookings: listOf(BOOKINGS),
      listCoaches: coachesOf(),
      listLessonMenus: lessonMenusOf(),
    });
    const user = userEvent.setup();
    render(<CancelledBookingsHome token="mock.headquarters.x" />);
    await screen.findByRole("table", { name: "キャンセルされた予約の一覧" });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "鈴木 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(replace).toHaveBeenCalledWith(`/headquarters/cancelled-bookings?coach_id=${SUZUKI}`, { scroll: false });
  });

  test("URL のクエリのレッスンメニューで開き、「この条件で見る」を押すとコーチと合わせて URL のクエリに残す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    headquartersApiMock.mockReturnValue({
      listCancelledBookings,
      listCoaches: coachesOf(),
      listLessonMenus: lessonMenusOf(),
    });
    const user = userEvent.setup();
    render(<CancelledBookingsHome token="mock.headquarters.x" initialLessonMenuId={FOREHAND} />);
    await screen.findByRole("table", { name: "キャンセルされた予約の一覧" });
    expect(listCancelledBookings).toHaveBeenCalledWith({ lesson_menu_id: FOREHAND });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "高橋 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(replace).toHaveBeenCalledWith(
      `/headquarters/cancelled-bookings?coach_id=${TAKAHASHI}&lesson_menu_id=${FOREHAND}`,
      { scroll: false },
    );
  });

  test("URL のクエリのスポーツで開き、「この条件で見る」を押すとコーチと合わせて URL のクエリに残す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    headquartersApiMock.mockReturnValue({
      listCancelledBookings,
      listCoaches: coachesOf(),
      listLessonMenus: lessonMenusOf(),
    });
    const user = userEvent.setup();
    render(<CancelledBookingsHome token="mock.headquarters.x" initialSport="tennis" />);
    await screen.findByRole("table", { name: "キャンセルされた予約の一覧" });
    expect(listCancelledBookings).toHaveBeenCalledWith({ sport: "tennis" });
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toBeEnabled());

    await user.selectOptions(screen.getByLabelText("コーチ"), "鈴木 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(replace).toHaveBeenCalledWith(`/headquarters/cancelled-bookings?coach_id=${SUZUKI}&sport=tennis`, {
      scroll: false,
    });
  });

  test("URL のクエリのキャンセルした人で開き、「この条件で見る」を押すと URL のクエリに残し、「すべて」に戻すと消す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    headquartersApiMock.mockReturnValue({
      listCancelledBookings,
      listCoaches: coachesOf(),
      listLessonMenus: lessonMenusOf(),
    });
    const user = userEvent.setup();
    render(<CancelledBookingsHome token="mock.headquarters.x" initialCancelledBy="headquarters" />);
    await screen.findByRole("table", { name: "キャンセルされた予約の一覧" });
    expect(listCancelledBookings).toHaveBeenCalledWith({ cancelled_by: "headquarters" });

    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(replace).toHaveBeenLastCalledWith("/headquarters/cancelled-bookings?cancelled_by=headquarters", {
      scroll: false,
    });

    await waitFor(() => expect(screen.getByRole("button", { name: "この条件で見る" })).toBeEnabled());
    await user.selectOptions(screen.getByLabelText("キャンセルした人"), "すべて");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(replace).toHaveBeenLastCalledWith("/headquarters/cancelled-bookings", { scroll: false });
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({
      listCancelledBookings: listOf(BOOKINGS),
      listCoaches: coachesOf(),
      listLessonMenus: lessonMenusOf(),
    });
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <CancelledBookingsHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "キャンセルされた予約" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
