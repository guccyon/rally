import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { BookingList } from "@/components/headquarters/BookingList";
import { BookingListHome } from "@/components/headquarters/BookingListHome";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

// 今は 2026-10-26 12:00（日本時間）。Date だけを止め、待ち合わせのタイマーは本物のまま。
const NOW = new Date("2026-10-26T03:00:00Z");
const HOUR = 3_600_000;
const at = (offsetHours: number) => new Date(NOW.getTime() + offsetHours * HOUR).toISOString();

beforeEach(() => {
  replace.mockReset();
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(NOW);
});

afterEach(() => {
  vi.useRealTimers();
});

function booking(
  n: number,
  overrides: Partial<Schemas["Booking"]>,
  names: { coach?: string; student?: string } = { coach: "鈴木 コーチ", student: "山田 花子" },
): Schemas["HeadquartersBooking"] {
  return {
    booking: {
      booking_id: `bbbbbbbb-bbbb-4bbb-8bbb-00000000000${n}`,
      slot_id: `55555555-5555-4555-8555-00000000000${n}`,
      student_id: "11111111-1111-4111-8111-000000000001",
      coach_id: "22222222-2222-4222-8222-000000000001",
      is_trial: false,
      status: "confirmed",
      starts_at: at(2),
      created_at: at(-48),
      confirmed_at: null,
      cancelled_at: null,
      lesson_menu: null,
      ...overrides,
    },
    ...(names.coach === undefined ? {} : { coach_display_name: names.coach }),
    ...(names.student === undefined ? {} : { student_display_name: names.student }),
  };
}

const BOOKINGS: Schemas["HeadquartersBooking"][] = [
  // 今日 14:00 開始の確定待ち（開始まで 2 時間）
  booking(1, {
    status: "pending",
    is_trial: true,
    starts_at: at(2),
    lesson_menu: { lesson_menu_id: "aaaaaaaa-aaaa-4aaa-8aaa-000000000001", name: "フォアハンドの基礎", duration_minutes: 60 },
  }),
  // 今日 18:00 開始の確定済み
  booking(2, { status: "confirmed", starts_at: at(6) }, { coach: "高橋 コーチ", student: "佐藤 一郎" }),
  // 明後日 12:00 開始の確定待ち（24 時間より先）
  booking(3, { status: "pending", starts_at: at(48) }, {}),
  // 明後日 15:00 開始のキャンセル済み
  booking(4, { status: "cancelled", starts_at: at(51) }),
];

function listOf(bookings: Schemas["HeadquartersBooking"][], period = { from: "2026-10-26", to: "2026-11-01" }) {
  return vi.fn<HeadquartersApi["listBookings"]>(async () => ({
    ok: true,
    data: { ...period, bookings },
  }));
}

function cellsOf(row: HTMLElement) {
  return within(row).getAllByRole("cell").map((cell) => cell.textContent);
}

function bodyRows(name: string) {
  return within(screen.getByRole("table", { name })).getAllByRole("row").slice(1);
}

describe("本部の予約", () => {
  test("既定は今日から 7 日（日本時間）で読み、期間の欄にも出す", async () => {
    const listBookings = listOf([]);
    render(<BookingList api={{ listBookings }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    expect(listBookings).toHaveBeenCalledWith({ from: "2026-10-26", to: "2026-11-01" });
    expect(screen.getByLabelText("開始日")).toHaveValue("2026-10-26");
    expect(screen.getByLabelText("終了日")).toHaveValue("2026-11-01");
    expect(await screen.findByText("2026年10月26日（月） 〜 2026年11月1日（日）・0 件")).toBeInTheDocument();
  });

  test("日本時間の日付が変わる前後でも今日は日本時間で決める", async () => {
    // UTC では 10/25 だが日本時間では 10/26 0:30
    vi.setSystemTime(new Date("2026-10-25T15:30:00Z"));
    const listBookings = listOf([]);
    render(<BookingList api={{ listBookings }} />);

    expect(listBookings).toHaveBeenCalledWith({ from: "2026-10-26", to: "2026-11-01" });
    await screen.findByText("この期間の予約はありません");
  });

  test("開始する日付ごとに分け、各行に開始・状態・コーチ・生徒・メニュー・種類を API の順で出す", async () => {
    render(<BookingList api={{ listBookings: listOf(BOOKINGS) }} />);

    await screen.findByRole("table", { name: "2026年10月26日（月） の予約" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "2026年10月26日（月）（2 件）" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "2026年10月28日（水）（2 件）" })).toBeInTheDocument();
    expect(bodyRows("2026年10月26日（月） の予約").map(cellsOf)).toEqual([
      ["14:00", "確定待ち開始まで 24 時間以内", "鈴木 コーチ", "山田 花子", "フォアハンドの基礎（60 分）", "体験予約"],
      ["18:00", "確定済み", "高橋 コーチ", "佐藤 一郎", "—", "—"],
    ]);
    expect(bodyRows("2026年10月28日（水） の予約").map(cellsOf)).toEqual([
      // 表示名が引けないときは「（名前を表示できません）」
      ["12:00", "確定待ち", "（名前を表示できません）", "（名前を表示できません）", "—", "—"],
      ["15:00", "キャンセル済み", "鈴木 コーチ", "山田 花子", "—", "—"],
    ]);
    expect(screen.getByText("2026年10月26日（月） 〜 2026年11月1日（日）・4 件")).toBeInTheDocument();
  });

  test("状態で色分けし、確定待ちのまま 24 時間以内に始まる予約だけを目立たせて件数を出す", async () => {
    render(<BookingList api={{ listBookings: listOf(BOOKINGS) }} />);
    await screen.findByRole("table", { name: "2026年10月26日（月） の予約" });

    expect(screen.getByText("確定待ちのまま 24 時間以内に始まる予約が 1 件あります。")).toBeInTheDocument();
    const [soon, confirmed] = bodyRows("2026年10月26日（月） の予約") as [HTMLElement, HTMLElement];
    const [laterPending, cancelled] = bodyRows("2026年10月28日（水） の予約") as [HTMLElement, HTMLElement];
    expect(soon).toHaveClass("bg-danger-faint");
    for (const row of [confirmed, laterPending, cancelled]) {
      expect(row).not.toHaveClass("bg-danger-faint");
      expect(within(row).queryByText("開始まで 24 時間以内")).not.toBeInTheDocument();
    }
    expect(within(soon).getByText("確定待ち")).toHaveClass("bg-warning-subtle");
    expect(within(laterPending).getByText("確定待ち")).toHaveClass("bg-warning-subtle");
    expect(within(confirmed).getByText("確定済み")).toHaveClass("bg-success-subtle");
    expect(within(cancelled).getByText("キャンセル済み")).toHaveClass("bg-muted");
  });

  test("始まってしまった確定待ちと、確定済みで近いものは目立たせない", async () => {
    const bookings = [
      booking(1, { status: "pending", starts_at: at(-1) }),
      booking(2, { status: "confirmed", starts_at: at(1) }),
      booking(3, { status: "pending", starts_at: at(24) }),
    ];
    render(<BookingList api={{ listBookings: listOf(bookings) }} />);
    await screen.findAllByRole("table");

    // ちょうど 24 時間後は含める
    expect(screen.getAllByText("開始まで 24 時間以内")).toHaveLength(1);
    expect(screen.getByText("確定待ちのまま 24 時間以内に始まる予約が 1 件あります。")).toBeInTheDocument();
  });

  test("0 件なら「この期間の予約はありません」と出し、表と注意は出さない", async () => {
    render(<BookingList api={{ listBookings: listOf([]) }} />);

    expect(await screen.findByText("この期間の予約はありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText(/24 時間以内に始まる予約/)).not.toBeInTheDocument();
  });

  test("期間を選び直して「この期間で見る」で読み直し、応答の期間を出す", async () => {
    const listBookings = listOf([]);
    const user = userEvent.setup();
    render(<BookingList api={{ listBookings }} />);
    await screen.findByText("この期間の予約はありません");

    listBookings.mockResolvedValueOnce({
      ok: true,
      data: { from: "2026-11-02", to: "2026-11-30", bookings: [booking(5, { starts_at: "2026-11-03T01:00:00Z" })] },
    });
    await user.clear(screen.getByLabelText("開始日"));
    await user.type(screen.getByLabelText("開始日"), "2026-11-02");
    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-11-30");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));

    expect(listBookings).toHaveBeenLastCalledWith({ from: "2026-11-02", to: "2026-11-30" });
    expect(await screen.findByText("2026年11月2日（月） 〜 2026年11月30日（月）・1 件")).toBeInTheDocument();
    expect(bodyRows("2026年11月3日（火） の予約").map((row) => cellsOf(row)[0])).toEqual(["10:00"]);
  });

  test("終了日が開始日より前・31 日を超える期間は送らずに理由を出す", async () => {
    const listBookings = listOf([]);
    const user = userEvent.setup();
    render(<BookingList api={{ listBookings }} />);
    await screen.findByText("この期間の予約はありません");

    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-10-25");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "終了日は開始日と同じか、それより後の日付にしてください。",
    );

    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-11-26");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("期間は 31 日までにしてください。");

    // 31 日ちょうど（10/26〜11/25）は送る
    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-11-25");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(listBookings).toHaveBeenCalledTimes(2);
    expect(listBookings).toHaveBeenLastCalledWith({ from: "2026-10-26", to: "2026-11-25" });
  });

  test("期間を続けて変えたときは最後の期間の結果だけを出す", async () => {
    let resolveFirst: (value: Awaited<ReturnType<HeadquartersApi["listBookings"]>>) => void = () => {};
    const listBookings = vi
      .fn<HeadquartersApi["listBookings"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-26", to: "2026-11-01", bookings: [] } })
      .mockImplementationOnce(() => new Promise((resolve) => (resolveFirst = resolve)))
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-27", to: "2026-10-27", bookings: [] } });
    const user = userEvent.setup();
    render(<BookingList api={{ listBookings }} />);
    await screen.findByText("この期間の予約はありません");

    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    await user.clear(screen.getByLabelText("開始日"));
    await user.type(screen.getByLabelText("開始日"), "2026-10-27");
    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-10-27");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    await screen.findByText("2026年10月27日（火） 〜 2026年10月27日（火）・0 件");

    resolveFirst({ ok: true, data: { from: "2026-10-26", to: "2026-11-01", bookings: BOOKINGS } });
    await waitFor(() => expect(screen.queryByRole("table")).not.toBeInTheDocument());
    expect(screen.getByText("2026年10月27日（火） 〜 2026年10月27日（火）・0 件")).toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listBookings = vi.fn<HeadquartersApi["listBookings"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<BookingList api={{ listBookings }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("この期間の予約はありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });
});

describe("画面への配置", () => {
  test("本部の画面の「予約」から予約の画面へ移れる", () => {
    render(<HeadquartersHome />);

    const region = screen.getByRole("region", { name: "予約" });
    expect(within(region).getByRole("link", { name: "予約の一覧を見る" })).toHaveAttribute(
      "href",
      "/headquarters/bookings",
    );
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <BookingListHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "予約" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
