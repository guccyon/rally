import { render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import type { PendingTasks } from "@/components/coach/pendingTasks";
import { CoachTasks } from "@/components/coach/CoachTasks";
import { ReturnVideoReviewForm } from "@/components/coach/ReturnVideoReviewForm";
import { videoLessonMenuLabel } from "@/components/coach/videoLessonMenu";

const FORM: Schemas["VideoLessonMenu"] = {
  lesson_menu_id: "a0000000-0000-4000-8000-000000000001",
  name: "フォーム添削",
};
const SERVE: Schemas["VideoLessonMenu"] = {
  lesson_menu_id: "a0000000-0000-4000-8000-000000000002",
  name: "サーブ動画チェック",
};

function coachVideo(
  id: string,
  studentName: string,
  returnedAt: string | undefined,
  lessonMenu: Schemas["VideoLessonMenu"] | null | undefined,
): Schemas["CoachVideo"] {
  return {
    video_id: id,
    student_display_name: studentName,
    submitted_at: "2026-10-20T01:00:00Z",
    review_status: returnedAt ? "returned" : "in_progress",
    ...(returnedAt ? { returned_at: returnedAt } : {}),
    ...(lessonMenu === undefined ? {} : { lesson_menu: lessonMenu }),
  };
}

// API の並び（添削待ち、その後に返却済み）。メニューあり・null・項目なし（古い版の応答）を混ぜる。
const VIDEOS = [
  coachVideo("b0000000-0000-4000-8000-000000000001", "佐藤さん", undefined, FORM),
  coachVideo("b0000000-0000-4000-8000-000000000002", "鈴木さん", undefined, null),
  coachVideo("b0000000-0000-4000-8000-000000000003", "中村さん", undefined, undefined),
  coachVideo("b0000000-0000-4000-8000-000000000004", "古田さん", "2026-10-25T01:00:00Z", SERVE),
  coachVideo("b0000000-0000-4000-8000-000000000005", "高橋さん", "2026-10-24T01:00:00Z", null),
];

function studentCell(table: HTMLElement, studentName: string) {
  const row = within(table).getByRole("row", { name: new RegExp(studentName) });
  return within(row).getAllByRole("cell")[0]!;
}

describe("動画のレッスンメニューの表示", () => {
  test("メニューがあれば「メニュー: 名前」、なければ null", () => {
    expect(videoLessonMenuLabel(FORM)).toBe("メニュー: フォーム添削");
    expect(videoLessonMenuLabel(null)).toBeNull();
    expect(videoLessonMenuLabel(undefined)).toBeNull();
  });
});

describe("動画の添削のレッスンメニュー", () => {
  test("添削待ち・返却済みの行に、メニューがあれば「メニュー: 〇〇」を出し、なければ名前だけ", async () => {
    render(
      <ReturnVideoReviewForm
        api={{
          listMyCoachVideos: vi.fn<CoachApi["listMyCoachVideos"]>(async () => ({
            ok: true,
            data: { videos: VIDEOS },
          })),
          returnVideoReview: vi.fn(),
        }}
      />,
    );

    const waiting = await screen.findByRole("table", { name: "添削待ち" });
    expect(studentCell(waiting, "佐藤さん")).toHaveTextContent(/^佐藤さんメニュー: フォーム添削$/);
    expect(studentCell(waiting, "鈴木さん")).toHaveTextContent(/^鈴木さん$/);
    expect(studentCell(waiting, "中村さん")).toHaveTextContent(/^中村さん$/);

    const done = screen.getByRole("table", { name: "返却済み" });
    expect(studentCell(done, "古田さん")).toHaveTextContent(/^古田さんメニュー: サーブ動画チェック$/);
    expect(studentCell(done, "高橋さん")).toHaveTextContent(/^高橋さん$/);
  });
});

describe("対応待ちの添削待ちの動画のレッスンメニュー", () => {
  test("メニューがあれば「メニュー: 〇〇」を出し、なければ今までどおり", () => {
    const waiting = VIDEOS.filter((video) => video.review_status === "in_progress");
    const tasks: PendingTasks = {
      videos: { ok: true, data: waiting },
      bookings: { ok: true, data: [] },
      threads: { ok: true, data: [] },
      names: new Map(),
    };
    render(
      <CoachTasks
        tasks={tasks}
        onOpenVideo={vi.fn()}
        onOpenBookings={vi.fn()}
        onOpenThread={vi.fn()}
      />,
    );

    const items = within(screen.getByRole("list", { name: "添削待ちの動画" })).getAllByRole("listitem");
    expect(items).toHaveLength(3);
    expect(items[0]).toHaveTextContent("メニュー: フォーム添削");
    expect(items[1]).toHaveTextContent("鈴木さん");
    expect(items[1]).not.toHaveTextContent("メニュー");
    expect(items[2]).not.toHaveTextContent("メニュー");
  });
});
