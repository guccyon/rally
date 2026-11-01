import { render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { StudentActivity } from "@/components/headquarters/StudentActivity";
import { StudentActivityHome } from "@/components/headquarters/StudentActivityHome";
import { StudentAnnouncements } from "@/components/headquarters/StudentAnnouncements";

vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace: vi.fn() }) }));

const STUDENT_ID = "11111111-1111-4111-8111-000000000001";

const ANNOUNCEMENTS: Schemas["StudentAnnouncementList"] = {
  announcements: [
    {
      announcement_id: "88888888-8888-4888-8888-000000000003",
      title: "年末年始の営業について",
      sent_at: "2026-10-30T01:00:00Z",
      is_read: false,
      is_withdrawn: false,
    },
    {
      announcement_id: "88888888-8888-4888-8888-000000000002",
      title: "メンテナンスのお知らせ",
      sent_at: "2026-10-28T09:30:00Z",
      is_read: true,
      is_withdrawn: false,
    },
    // 取り下げたお知らせも載る（既読は取り下げても変わらない）
    {
      announcement_id: "88888888-8888-4888-8888-000000000001",
      title: "テニスの新しいコーチのご紹介",
      sent_at: "2026-10-27T00:00:00Z",
      is_read: true,
      is_withdrawn: true,
    },
  ],
};

function apiOf(result: Result<Schemas["StudentAnnouncementList"]>) {
  return {
    listStudentAnnouncements: vi.fn<HeadquartersApi["listStudentAnnouncements"]>(async () => result),
  };
}

function rows() {
  return within(screen.getByRole("table", { name: "受け取ったお知らせ" }))
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent));
}

describe("本部の生徒の受け取ったお知らせ", () => {
  test("読み込み中を出し、読めたら返った順（送った新しい順）に題・送った日時・既読・取り下げを出す", async () => {
    const api = apiOf({ ok: true, data: ANNOUNCEMENTS });
    render(<StudentAnnouncements api={api} studentId={STUDENT_ID} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    await screen.findByRole("table", { name: "受け取ったお知らせ" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(api.listStudentAnnouncements).toHaveBeenCalledTimes(1);
    expect(api.listStudentAnnouncements).toHaveBeenCalledWith(STUDENT_ID);

    expect(rows()).toEqual([
      ["年末年始の営業について", "2026/10/30 10:00:00", "未読", "—"],
      ["メンテナンスのお知らせ", "2026/10/28 18:30:00", "既読", "—"],
      ["テニスの新しいコーチのご紹介", "2026/10/27 9:00:00", "既読", "取り下げ済み"],
    ]);
    expect(screen.getByText("取り下げたお知らせは生徒のアプリの一覧には出ていません。", { exact: false })).toBeInTheDocument();
  });

  test("0 件ならその旨を出し、表は出さない", async () => {
    render(<StudentAnnouncements api={apiOf({ ok: true, data: { announcements: [] } })} studentId={STUDENT_ID} />);

    expect(await screen.findByText("受け取ったお知らせはありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("読み込めなければ欄の中にその文言を出し、表は出さない", async () => {
    render(
      <StudentAnnouncements
        api={apiOf({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" })}
        studentId={STUDENT_ID}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "受け取ったお知らせを読めませんでした。権限がないためアクセスできません。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("お知らせが読めなくても、利用の様子の欄は出し続ける", async () => {
    const getStudentActivity = vi.fn<HeadquartersApi["getStudentActivity"]>(async () => ({
      ok: true,
      data: {
        student_id: STUDENT_ID,
        display_name: "山田 花子",
        upcoming_bookings: [],
        recent_lessons: [],
        cancelled_booking_count: 0,
      },
    }));
    render(
      <>
        <StudentActivity api={{ getStudentActivity }} studentId={STUDENT_ID} />
        <StudentAnnouncements
          api={apiOf({ ok: false, message: "時間をおいてもう一度お試しください。" })}
          studentId={STUDENT_ID}
        />
      </>,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent("受け取ったお知らせを読めませんでした。");
    expect(await screen.findByText("山田 花子")).toBeInTheDocument();
    expect(screen.getByText("これからの予約はありません")).toBeInTheDocument();
  });
});

describe("画面への配置", () => {
  test("生徒の利用の様子の画面に「受け取ったお知らせ」の欄を出す。生徒の ID がなければ出さない", () => {
    const { unmount } = render(<StudentActivityHome token="mock.headquarters.abc" studentId={STUDENT_ID} />);
    expect(screen.getByRole("region", { name: "生徒の利用の様子" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "受け取ったお知らせ" })).toBeInTheDocument();
    unmount();

    render(<StudentActivityHome token="mock.headquarters.abc" studentId={undefined} />);
    expect(screen.queryByRole("region", { name: "受け取ったお知らせ" })).not.toBeInTheDocument();
  });
});
