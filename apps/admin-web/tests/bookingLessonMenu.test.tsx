import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { bookingLessonMenuLabel } from "@/components/coach/bookingLessonMenu";
import { CoachHome } from "@/components/coach/CoachHome";
import { LiveSessionPanel } from "@/components/coach/LiveSessionPanel";

// 配置のテストでは CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const STUDENT_ID = "33333333-3333-4333-8333-333333333333";
const HOUR = 60 * 60 * 1000;
const at = (offsetHours: number) => new Date(Date.now() + offsetHours * HOUR).toISOString();

const SERVE: Schemas["BookingLessonMenu"] = {
  lesson_menu_id: "a0000000-0000-4000-8000-000000000001",
  name: "サーブ特訓",
  duration_minutes: 90,
};
// 予約した時点でメニューが非同期に直されていた（時間がない）。
const NO_DURATION: Schemas["BookingLessonMenu"] = {
  lesson_menu_id: "a0000000-0000-4000-8000-000000000002",
  name: "フォーム改善",
  duration_minutes: null,
};

function coachBooking(
  id: string,
  startsIn: number,
  studentName: string,
  lessonMenu: Schemas["BookingLessonMenu"] | null | undefined,
): Schemas["CoachBooking"] {
  const slotId = id.replace(/^./, "1");
  return {
    booking: {
      booking_id: id,
      slot_id: slotId,
      student_id: STUDENT_ID,
      coach_id: COACH_ID,
      is_trial: false,
      status: "confirmed",
      starts_at: at(startsIn),
      created_at: at(-100),
      confirmed_at: at(-99),
      cancelled_at: null,
      ...(lessonMenu === undefined ? {} : { lesson_menu: lessonMenu }),
    },
    slot: {
      slot_id: slotId,
      coach_id: COACH_ID,
      sport: "tennis",
      starts_at: at(startsIn),
      ends_at: at(startsIn + 1.5),
      status: "booked",
      capacity: 1,
      remaining_seats: 0,
    },
    student_display_name: studentName,
  };
}

// API の並び（開始時刻の古い順）。終わった 1 件とこれからの 3 件。
const BOOKINGS = [
  coachBooking("b0000000-0000-4000-8000-000000000001", -48, "古田さん", SERVE),
  coachBooking("b0000000-0000-4000-8000-000000000002", 24, "佐藤さん", SERVE),
  coachBooking("b0000000-0000-4000-8000-000000000003", 48, "鈴木さん", NO_DURATION),
  coachBooking("b0000000-0000-4000-8000-000000000004", 72, "中村さん", null),
];

function listOf(bookings: Schemas["CoachBooking"][]) {
  return vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({ ok: true, data: { bookings } }));
}

function menuCell(table: HTMLElement, studentName: string) {
  const row = within(table).getByRole("row", { name: new RegExp(studentName) });
  return within(row).getAllByRole("cell")[2]!;
}

describe("予約のレッスンメニューの表示", () => {
  test("名前と時間を「名前（N 分）」で、時間がなければ名前だけ、メニューがなければ「—」", () => {
    expect(bookingLessonMenuLabel(SERVE)).toBe("サーブ特訓（90 分）");
    expect(bookingLessonMenuLabel(NO_DURATION)).toBe("フォーム改善");
    expect(bookingLessonMenuLabel(null)).toBe("—");
    expect(bookingLessonMenuLabel(undefined)).toBe("—");
  });
});

describe("通話への参加の予約のメニュー", () => {
  test("これからの予約と終わった予約の行に、予約したときのメニューの名前と時間を出す", async () => {
    render(
      <LiveSessionPanel
        api={{
          listMyCoachBookings: listOf(BOOKINGS),
          startLiveLessonSession: vi.fn(),
          endLiveLessonSession: vi.fn(),
        }}
      />,
    );

    const upcoming = await screen.findByRole("table", { name: "これからの予約" });
    expect(within(upcoming).getByRole("columnheader", { name: "メニュー" })).toBeInTheDocument();
    expect(menuCell(upcoming, "佐藤さん")).toHaveTextContent(/^サーブ特訓（90 分）$/);
    expect(menuCell(upcoming, "鈴木さん")).toHaveTextContent(/^フォーム改善$/);
    expect(menuCell(upcoming, "中村さん")).toHaveTextContent(/^—$/);

    const past = screen.getByRole("table", { name: "終わった予約" });
    expect(menuCell(past, "古田さん")).toHaveTextContent(/^サーブ特訓（90 分）$/);
  });

  test("lesson_menu を持たない予約（古い版の応答）も、メニューのない予約と同じく「—」で、通話は今までどおり始められる", async () => {
    render(
      <LiveSessionPanel
        api={{
          listMyCoachBookings: listOf([
            coachBooking("b0000000-0000-4000-8000-000000000005", 24, "佐藤さん", undefined),
          ]),
          startLiveLessonSession: vi.fn(),
          endLiveLessonSession: vi.fn(),
        }}
      />,
    );

    const upcoming = await screen.findByRole("table", { name: "これからの予約" });
    expect(menuCell(upcoming, "佐藤さん")).toHaveTextContent(/^—$/);
    expect(within(upcoming).getByRole("button", { name: "通話を開始" })).toBeEnabled();
  });
});

describe("コーチの画面の「通話への参加」", () => {
  beforeEach(() => {
    coachApiMock.mockReset();
  });

  test("通話への参加のタブで、予約の行にメニューが出る", async () => {
    coachApiMock.mockReturnValue({
      listMyCoachBookings: listOf(BOOKINGS),
      startLiveLessonSession: vi.fn(),
      endLiveLessonSession: vi.fn(),
      // 開いたときに出す「対応待ち」が呼ぶ。
      listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
        ok: true,
        data: { videos: [] },
      })),
      listMessageThreads: vi.fn<CoachApi["listMessageThreads"]>(async () => ({
        ok: true,
        data: { threads: [] },
      })),
    });
    const user = userEvent.setup();
    render(<CoachHome token="mock.coach.x" />);
    await user.click(screen.getByRole("tab", { name: "通話への参加" }));
    const region = screen.getByRole("region", { name: "通話への参加" });

    const upcoming = await within(region).findByRole("table", { name: "これからの予約" });
    expect(menuCell(upcoming, "佐藤さん")).toHaveTextContent(/^サーブ特訓（90 分）$/);
    expect(menuCell(upcoming, "中村さん")).toHaveTextContent(/^—$/);
  });
});
