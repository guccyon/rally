import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { CoachHome } from "@/components/coach/CoachHome";
import { formatDateTime } from "@/components/labels";

import { EMPTY_LESSON_STATS } from "./lessonStatsFixtures";

// CoachHome が作る API を差し替える（Node の Request は相対 URL の "/api" を解決しない）。
const { coachApiMock } = vi.hoisted(() => ({ coachApiMock: vi.fn() }));
vi.mock("@/api/coach", () => ({ coachApi: coachApiMock }));

function notification(
  n: number,
  kind: Schemas["NotificationKind"],
  body: string,
  isRead: boolean,
): Schemas["Notification"] {
  return {
    notification_id: `99999999-9999-4999-8999-99999999999${n}`,
    kind,
    body,
    created_at: `2026-10-2${n}T03:00:00Z`,
    is_read: isRead,
  };
}

// API が返す並び（作った時刻の新しい順）。未読 2 件・既読 1 件。
const NOTIFICATIONS = [
  notification(3, "message_received", "生徒から新しいメッセージが届きました", false),
  notification(2, "video_review_requested", "生徒から動画の添削の依頼が届きました", false),
  notification(1, "booking_confirmed", "新しい予約が入りました", true),
];

const allRead = (notifications: Schemas["Notification"][]) =>
  notifications.map((item) => ({ ...item, is_read: true }));

function mockApi(overrides: Partial<CoachApi> = {}) {
  const api = {
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
    listMyNotifications: vi.fn<CoachApi["listMyNotifications"]>(async () => ({
      ok: true,
      data: { notifications: NOTIFICATIONS },
    })),
    markMyNotificationsRead: vi.fn<CoachApi["markMyNotificationsRead"]>(async () => ({
      ok: true,
      data: { notifications: allRead(NOTIFICATIONS) },
    })),
    ...overrides,
  };
  coachApiMock.mockReturnValue(api);
  return api;
}

function setup(overrides: Partial<CoachApi> = {}) {
  const api = mockApi(overrides);
  const user = userEvent.setup();
  render(<CoachHome token="mock.coach.x" />);
  return { api, user };
}

function notificationsButton() {
  return screen.getByRole("button", { name: /^お知らせ/ });
}

beforeEach(() => {
  coachApiMock.mockReset();
});

describe("コーチのお知らせ", () => {
  test("見出しの「お知らせ」に未読のお知らせの数のバッジを出す", async () => {
    setup();

    const button = await screen.findByRole("button", { name: "お知らせ（未読 2 件）" });
    expect(button).toHaveTextContent(/^お知らせ2$/);
  });

  test("未読がなければバッジを出さない", async () => {
    const { api } = setup({
      listMyNotifications: vi.fn<CoachApi["listMyNotifications"]>(async () => ({
        ok: true,
        data: { notifications: allRead(NOTIFICATIONS) },
      })),
    });

    await vi.waitFor(() => expect(api.listMyNotifications).toHaveBeenCalled());
    expect(notificationsButton()).toHaveTextContent(/^お知らせ$/);
    expect(notificationsButton()).not.toHaveAccessibleName(/未読/);
  });

  test("押すと一覧を新しい順に出し、開いたときに未読だったものに「新着」と添えて、すべて既読にする", async () => {
    const { api, user } = setup();
    await screen.findByRole("button", { name: "お知らせ（未読 2 件）" });

    await user.click(notificationsButton());

    const region = screen.getByRole("region", { name: "お知らせ" });
    const list = await within(region).findByRole("list", { name: "お知らせの一覧" });
    const items = within(list).getAllByRole("listitem");
    expect(items).toHaveLength(3);
    expect(items[0]).toHaveTextContent("生徒から新しいメッセージが届きました");
    expect(items[0]).toHaveTextContent(formatDateTime(NOTIFICATIONS[0]!.created_at));
    expect(items[1]).toHaveTextContent("生徒から動画の添削の依頼が届きました");
    expect(items[2]).toHaveTextContent("新しい予約が入りました");
    expect(within(items[0]!).getByText("新着")).toBeInTheDocument();
    expect(within(items[1]!).getByText("新着")).toBeInTheDocument();
    expect(within(items[2]!).queryByText("新着")).not.toBeInTheDocument();

    await vi.waitFor(() => expect(api.markMyNotificationsRead).toHaveBeenCalledTimes(1));
    // 既読にしたあとも「新着」は残し、見出しのバッジは消す
    await vi.waitFor(() => expect(notificationsButton()).toHaveTextContent(/^お知らせ$/));
    expect(within(items[0]!).getByText("新着")).toBeInTheDocument();
    // 一覧を開いている間はメニューのタブを選んでいない
    expect(screen.getByRole("tab", { name: /対応待ち/ })).toHaveAttribute("aria-selected", "false");
  });

  test("未読がなければ既読にする API を呼ばない", async () => {
    const { api, user } = setup({
      listMyNotifications: vi.fn<CoachApi["listMyNotifications"]>(async () => ({
        ok: true,
        data: { notifications: allRead(NOTIFICATIONS) },
      })),
    });

    await user.click(notificationsButton());

    const list = await screen.findByRole("list", { name: "お知らせの一覧" });
    expect(within(list).queryByText("新着")).not.toBeInTheDocument();
    expect(api.markMyNotificationsRead).not.toHaveBeenCalled();
  });

  test.each([
    ["生徒から新しいメッセージが届きました", "チャット"],
    ["生徒から動画の添削の依頼が届きました", "動画の添削"],
    ["新しい予約が入りました", "通話への参加"],
  ])("「%s」の行を押すと「%s」へ移る", async (body, tabName) => {
    const { user } = setup();
    await user.click(notificationsButton());
    const list = await screen.findByRole("list", { name: "お知らせの一覧" });

    await user.click(within(list).getByRole("button", { name: new RegExp(body) }));

    expect(screen.getByRole("tab", { name: new RegExp(`^${tabName}`) })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("region", { name: tabName })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "お知らせ" })).not.toBeInTheDocument();
  });

  test("このアプリが知らない種類の行は押せない", async () => {
    const unknown = {
      ...notification(4, "booking_confirmed", "新しいお知らせの種類です", false),
      kind: "something_new",
    } as unknown as Schemas["Notification"];
    const { user } = setup({
      listMyNotifications: vi.fn<CoachApi["listMyNotifications"]>(async () => ({
        ok: true,
        data: { notifications: [unknown] },
      })),
      markMyNotificationsRead: vi.fn<CoachApi["markMyNotificationsRead"]>(async () => ({
        ok: true,
        data: { notifications: allRead([unknown]) },
      })),
    });
    await user.click(notificationsButton());

    const list = await screen.findByRole("list", { name: "お知らせの一覧" });
    expect(within(list).getByText("新しいお知らせの種類です")).toBeInTheDocument();
    expect(within(list).queryByRole("button")).not.toBeInTheDocument();
  });

  test("お知らせがなければない旨を出す", async () => {
    const { user } = setup({
      listMyNotifications: vi.fn<CoachApi["listMyNotifications"]>(async () => ({
        ok: true,
        data: { notifications: [] },
      })),
    });
    await user.click(notificationsButton());

    const region = screen.getByRole("region", { name: "お知らせ" });
    expect(await within(region).findByText(/まだお知らせはありません。/)).toBeInTheDocument();
  });

  test("一覧が読めなければその文言だけを出す", async () => {
    const { user } = setup({
      listMyNotifications: vi.fn<CoachApi["listMyNotifications"]>(async () => ({
        ok: false,
        message: "サーバーに接続できません。",
      })),
    });
    await user.click(notificationsButton());

    const region = screen.getByRole("region", { name: "お知らせ" });
    expect(await within(region).findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(within(region).queryByRole("list")).not.toBeInTheDocument();
  });

  test("既読にできなくても一覧は出し、未読の数は残す", async () => {
    const { api, user } = setup({
      markMyNotificationsRead: vi.fn<CoachApi["markMyNotificationsRead"]>(async () => ({
        ok: false,
        message: "サーバーに接続できません。",
      })),
    });
    await screen.findByRole("button", { name: "お知らせ（未読 2 件）" });
    await user.click(notificationsButton());

    const list = await screen.findByRole("list", { name: "お知らせの一覧" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(3);
    await vi.waitFor(() => expect(api.markMyNotificationsRead).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "お知らせ（未読 2 件）" })).toBeInTheDocument();
  });
});
