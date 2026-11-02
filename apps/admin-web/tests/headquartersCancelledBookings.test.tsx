import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { CancelledBookingList } from "@/components/headquarters/CancelledBookingList";
import { CancelledBookingsHome } from "@/components/headquarters/CancelledBookingsHome";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { formatDateTime } from "@/components/labels";
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
    render(<CancelledBookingList api={{ listCancelledBookings }} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
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
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
  });

  test("API の既定で実際に使った期間を件数とともに出し、期間の欄にも入れる", async () => {
    render(<CancelledBookingList api={{ listCancelledBookings: listOf(BOOKINGS) }} />);

    expect(await screen.findByText("2026年10月20日（火） 〜 2026年10月26日（月）・2 件")).toBeInTheDocument();
    expect(screen.getByLabelText("開始日")).toHaveValue("2026-10-20");
    expect(screen.getByLabelText("終了日")).toHaveValue("2026-10-26");
  });

  test("期間を選んで「この期間で見る」で API の from / to に渡して読み直す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-10-01", "2026-10-31");
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    expect(listCancelledBookings).toHaveBeenLastCalledWith({ from: "2026-10-01", to: "2026-10-31" });
  });

  test("終了日が開始日より前・31 日を超える期間は送らずに理由を出す", async () => {
    const listCancelledBookings = listOf(BOOKINGS);
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-10-20", "2026-10-19");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "終了日は開始日と同じか、それより後の日付にしてください。",
    );

    await changePeriod(user, "2026-09-01", "2026-10-02");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("期間は 31 日までにしてください。");
    expect(listCancelledBookings).toHaveBeenCalledTimes(1);
  });

  test("API の 422 は、期間のどこを直すかを添えた理由を出し、一覧は出さない", async () => {
    const listCancelledBookings = vi.fn<HeadquartersApi["listCancelledBookings"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力の内容が正しくありません。",
    }));
    render(<CancelledBookingList api={{ listCancelledBookings }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "期間の指定が正しくありません。期間は 1〜31 日で、終了日は開始日と同じかそれより後の日付にしてください。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("後から変えた期間の結果だけを出す（前の期間の遅れた結果で上書きしない）", async () => {
    let resolveSlow: (value: Response) => void = () => {};
    const listCancelledBookings = vi
      .fn<HeadquartersApi["listCancelledBookings"]>()
      .mockResolvedValueOnce({ ok: true, data: { from: "2026-10-20", to: "2026-10-26", bookings: BOOKINGS } })
      .mockImplementationOnce(() => new Promise((resolve) => (resolveSlow = resolve)))
      .mockResolvedValueOnce({
        ok: true,
        data: { from: "2026-10-25", to: "2026-10-26", bookings: [BOOKINGS[0]!] },
      });
    const user = userEvent.setup();
    render(<CancelledBookingList api={{ listCancelledBookings }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-10-01", "2026-10-10");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    await changePeriod(user, "2026-10-25", "2026-10-26");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    resolveSlow({ ok: true, data: { from: "2026-10-01", to: "2026-10-10", bookings: [BOOKINGS[1]!] } });

    await waitFor(() => expect(screen.getAllByRole("row")).toHaveLength(2));
    expect(screen.getByRole("row", { name: /佐藤 太郎/ })).toBeInTheDocument();
  });

  test("0 件なら「この期間にキャンセルされた予約はありません」と出し、表は出さない", async () => {
    render(<CancelledBookingList api={{ listCancelledBookings: listOf([]) }} />);

    expect(await screen.findByText("この期間にキャンセルされた予約はありません")).toBeInTheDocument();
    expect(screen.getByText("2026年10月20日（火） 〜 2026年10月26日（月）・0 件")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listCancelledBookings = vi.fn<HeadquartersApi["listCancelledBookings"]>(async () => ({
      ok: false,
      code: "forbidden",
      message: "権限がないためアクセスできません。",
    }));
    render(<CancelledBookingList api={{ listCancelledBookings }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByText("この期間にキャンセルされた予約はありません")).not.toBeInTheDocument();
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
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
    headquartersApiMock.mockReturnValue({ listCancelledBookings: listOf(BOOKINGS) });
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

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    headquartersApiMock.mockReturnValue({ listCancelledBookings: listOf(BOOKINGS) });
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
