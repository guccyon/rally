import { render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { BookingDetail } from "@/components/headquarters/BookingDetail";
import { BookingDetailHome, bookingDetailPath } from "@/components/headquarters/BookingDetailHome";
import { BookingList } from "@/components/headquarters/BookingList";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const BOOKING_ID = "bbbbbbbb-bbbb-4bbb-8bbb-000000000001";
const SLOT_ID = "55555555-5555-4555-8555-000000000001";
const STUDENT_ID = "11111111-1111-4111-8111-000000000001";
const COACH_ID = "22222222-2222-4222-8222-000000000001";

const BOOKING: Schemas["Booking"] = {
  booking_id: BOOKING_ID,
  slot_id: SLOT_ID,
  student_id: STUDENT_ID,
  coach_id: COACH_ID,
  is_trial: true,
  status: "cancelled",
  starts_at: "2026-10-27T01:00:00Z",
  created_at: "2026-10-20T00:00:00Z",
  confirmed_at: "2026-10-20T01:00:00Z",
  cancelled_at: "2026-10-22T09:30:00Z",
  lesson_menu: { lesson_menu_id: "aaaaaaaa-aaaa-4aaa-8aaa-000000000001", name: "フォアハンドの基礎", duration_minutes: 60 },
};

const SLOT: Schemas["Slot"] = {
  slot_id: SLOT_ID,
  coach_id: COACH_ID,
  sport: "tennis",
  starts_at: "2026-10-27T01:00:00Z",
  ends_at: "2026-10-27T02:00:00Z",
  status: "open",
  capacity: 1,
  remaining_seats: 1,
};

const DETAIL: Schemas["HeadquartersBookingDetail"] = {
  booking: BOOKING,
  slot: SLOT,
  status_changes: [
    { status: "confirmed", changed_at: "2026-10-20T01:00:00Z" },
    { status: "cancelled", changed_at: "2026-10-22T09:30:00Z" },
  ],
  coach_display_name: "鈴木 コーチ",
  student_display_name: "山田 花子",
};

function detailOf(detail: Schemas["HeadquartersBookingDetail"]) {
  return vi.fn<HeadquartersApi["getBooking"]>(async () => ({ ok: true, data: detail }));
}

/** 説明の並び（dt と dd）を [見出し, 値] の組にする。 */
function termsOf() {
  const terms = screen.getAllByRole("term").map((term) => term.textContent);
  const values = screen.getAllByRole("definition").map((value) => value.textContent);
  return terms.map((term, index) => [term, values[index]]);
}

function rowsOf(name: string) {
  return within(screen.getByRole("table", { name }))
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent));
}

describe("本部の予約の詳細", () => {
  test("読み込み中を出し、読めたら生徒・コーチ・メニュー・枠の時刻・種類・今の状態と、状態が変わった記録を API の順で出す", async () => {
    const getBooking = detailOf(DETAIL);
    render(<BookingDetail api={{ getBooking }} bookingId={BOOKING_ID} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    await screen.findByRole("table", { name: "状態が変わった記録" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(getBooking).toHaveBeenCalledWith(BOOKING_ID);
    expect(getBooking).toHaveBeenCalledTimes(1);

    expect(termsOf()).toEqual([
      ["生徒", "山田 花子"],
      ["コーチ", "鈴木 コーチ"],
      ["レッスンメニュー", "フォアハンドの基礎（60 分）"],
      ["枠の時刻", "2026/10/27 10:00:00 〜 11:00"],
      ["種類", "体験予約"],
      ["今の状態", "キャンセル済み"],
      ["予約した日時", "2026/10/20 9:00:00"],
    ]);
    expect(rowsOf("状態が変わった記録")).toEqual([
      ["2026/10/20 10:00:00", "確定済み"],
      ["2026/10/22 18:30:00", "キャンセル済み"],
    ]);
  });

  test("今の状態と記録の状態は、予約の一覧と同じ色の札で出す", async () => {
    render(<BookingDetail api={{ getBooking: detailOf(DETAIL) }} bookingId={BOOKING_ID} />);

    const table = await screen.findByRole("table", { name: "状態が変わった記録" });
    const current = screen.getAllByRole("definition")[5]!;
    expect(within(current).getByText("キャンセル済み")).toHaveClass("bg-gray-100", "text-gray-500");
    expect(within(table).getByText("確定済み")).toHaveClass("bg-green-100", "text-green-800");
    expect(within(table).getByText("キャンセル済み")).toHaveClass("bg-gray-100", "text-gray-500");
  });

  test("名前が引けない・メニューがない・体験予約でない・日をまたぐ枠は、それぞれの形で出す", async () => {
    render(
      <BookingDetail
        api={{
          getBooking: detailOf({
            booking: { ...BOOKING, is_trial: false, status: "pending", lesson_menu: null },
            slot: { ...SLOT, starts_at: "2026-10-27T14:30:00Z", ends_at: "2026-10-27T15:30:00Z" },
            status_changes: [],
          }),
        }}
        bookingId={BOOKING_ID}
      />,
    );

    await screen.findByText("状態が変わった記録はありません");
    expect(termsOf()).toEqual([
      ["生徒", "（名前を表示できません）"],
      ["コーチ", "（名前を表示できません）"],
      ["レッスンメニュー", "—"],
      ["枠の時刻", "2026/10/27 23:30:00 〜 2026/10/28 0:30:00"],
      ["種類", "—"],
      ["今の状態", "確定待ち"],
      ["予約した日時", "2026/10/20 9:00:00"],
    ]);
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("API が 404 なら「見つからない」を出す", async () => {
    const getBooking = vi.fn<HeadquartersApi["getBooking"]>(async () => ({
      ok: false,
      code: "not_found",
      message: "対象が見つかりません（ID を確かめてください）。",
    }));
    render(<BookingDetail api={{ getBooking }} bookingId="not-a-uuid" />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "この予約は見つかりません。予約の一覧から選び直してください。",
    );
    expect(screen.queryByText("生徒")).not.toBeInTheDocument();
  });

  test("予約の ID がなければ API を呼ばずに「見つからない」を出す", () => {
    const getBooking = detailOf(DETAIL);
    render(<BookingDetail api={{ getBooking }} bookingId={undefined} />);

    expect(screen.getByRole("alert")).toHaveTextContent("この予約は見つかりません。");
    expect(getBooking).not.toHaveBeenCalled();
  });

  test("404 以外の失敗（403 など）はその文言を出し、中身は出さない", async () => {
    const getBooking = vi.fn<HeadquartersApi["getBooking"]>(async () => ({
      ok: false,
      code: "forbidden",
      message: "権限がないためアクセスできません。",
    }));
    render(<BookingDetail api={{ getBooking }} bookingId={BOOKING_ID} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByText("この予約は見つかりません。", { exact: false })).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument());
  });
});

describe("画面への配置", () => {
  test("予約の一覧の各行の開始の時刻から、その予約の詳細へ移れる", async () => {
    const listBookings = vi.fn<HeadquartersApi["listBookings"]>(async () => ({
      ok: true,
      data: {
        from: "2026-10-26",
        to: "2026-11-01",
        bookings: [{ booking: BOOKING, coach_display_name: "鈴木 コーチ", student_display_name: "山田 花子" }],
      },
    }));
    render(<BookingList api={{ listBookings }} />);

    expect(await screen.findByRole("link", { name: "10:00" })).toHaveAttribute(
      "href",
      `/headquarters/bookings/detail?booking_id=${BOOKING_ID}`,
    );
    expect(bookingDetailPath(BOOKING_ID)).toBe(`/headquarters/bookings/detail?booking_id=${BOOKING_ID}`);
  });

  test("予約の詳細から予約の一覧へ戻れる", () => {
    saveSession({ role: "headquarters", token: "mock.headquarters.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <BookingDetailHome token={session.token} bookingId={undefined} />}
      </RoleGate>,
    );

    expect(screen.getByRole("link", { name: "予約の一覧へ戻る" })).toHaveAttribute("href", "/headquarters/bookings");
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <BookingDetailHome token={session.token} bookingId={BOOKING_ID} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "予約の詳細" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
