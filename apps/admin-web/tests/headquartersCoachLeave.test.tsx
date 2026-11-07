import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { CoachLeave, coachLeavePath } from "@/components/headquarters/CoachLeaveHome";
import { CoachRelatedLinks } from "@/components/headquarters/CoachRelatedLinks";
import { leavePeriodErrors, periodLabel, slotTimeLabel } from "@/components/headquarters/coachLeave";

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, replace: vi.fn() }) }));

const COACH_ID = "22222222-2222-4222-8222-000000000001";
const BOOKING_A = "33333333-3333-4333-8333-000000000001";
const BOOKING_B = "33333333-3333-4333-8333-000000000002";

/** 2026-11-05T12:00:00+09:00（今日は 11/5） */
const NOW = Date.parse("2026-11-05T03:00:00Z");

const OVERVIEW = {
  coach_id: COACH_ID,
  display_name: "山田 健",
  acceptance_status: "accepting",
} as Schemas["CoachOverview"];

const PREVIEW: Schemas["CoachLeavePreview"] = {
  coach: { coach_id: COACH_ID, display_name: "山田 健", sports: ["tennis"], acceptance_status: "accepting" },
  from: "2026-11-10",
  to: "2026-11-16",
  bookings: [
    {
      booking_id: BOOKING_A,
      slot_id: "44444444-4444-4444-8444-000000000001",
      starts_at: "2026-11-10T10:00:00Z",
      ends_at: "2026-11-10T10:30:00Z",
      status: "confirmed",
      is_trial: true,
      student_id: "55555555-5555-4555-8555-000000000001",
      student_display_name: "佐藤 花子",
    },
    {
      booking_id: BOOKING_B,
      slot_id: "44444444-4444-4444-8444-000000000002",
      starts_at: "2026-11-12T12:00:00Z",
      ends_at: "2026-11-12T12:30:00Z",
      status: "pending",
      is_trial: false,
      lesson_menu_name: "フォアハンドの見直し",
      student_id: "55555555-5555-4555-8555-000000000002",
      student_display_name: "高橋 美咲",
    },
  ],
  open_slots: [
    {
      slot_id: "44444444-4444-4444-8444-000000000003",
      starts_at: "2026-11-15T00:00:00Z",
      ends_at: "2026-11-15T00:30:00Z",
      capacity: 1,
    },
  ],
  slot_count: 3,
  student_count: 2,
  notification_preview: {
    title: "コーチのお休みで予約をキャンセルしました",
    body: "山田 健 コーチのお休みのため、11/10（火）19:00〜19:30 の予約を本部でキャンセルしました。",
  },
};

const RESULT: Schemas["CoachLeaveResult"] = {
  coach_leave: {
    coach_leave_id: "66666666-6666-4666-8666-000000000001",
    coach_id: COACH_ID,
    from: "2026-11-10",
    to: "2026-11-16",
    note: "",
    created_by: "77777777-7777-4777-8777-000000000001",
    created_at: "2026-11-05T03:00:00Z",
  },
  cancelled_booking_ids: [BOOKING_A, BOOKING_B],
  cancelled_booking_count: 2,
  notified_student_count: 2,
  withdrawn_slot_count: 3,
};

function apiOf({
  preview = { ok: true, data: PREVIEW },
  start = { ok: true, data: RESULT },
}: {
  preview?: Result<Schemas["CoachLeavePreview"]>;
  start?: Result<Schemas["CoachLeaveResult"]>;
} = {}) {
  return {
    getCoachOverview: vi.fn<HeadquartersApi["getCoachOverview"]>(async () => ({ ok: true, data: OVERVIEW })),
    getCoachLeavePreview: vi.fn<HeadquartersApi["getCoachLeavePreview"]>(async () => preview),
    startCoachLeave: vi.fn<HeadquartersApi["startCoachLeave"]>(async () => start),
  };
}

async function fillPeriod(user: ReturnType<typeof userEvent.setup>, from: string, to: string) {
  const fromField = screen.getByLabelText("始まりの日");
  const toField = screen.getByLabelText("終わりの日");
  await user.clear(fromField);
  if (from) await user.type(fromField, from);
  await user.clear(toField);
  if (to) await user.type(toField, to);
}

async function openWithPreview(api: ReturnType<typeof apiOf>) {
  const user = userEvent.setup();
  render(<CoachLeave api={api} coachId={COACH_ID} now={() => NOW} />);
  expect(await screen.findByText("山田 健")).toBeInTheDocument();
  await fillPeriod(user, "2026-11-10", "2026-11-16");
  await user.click(screen.getByRole("button", { name: "期間の予約と枠を確かめる" }));
  await screen.findByRole("list", { name: "期間の予約の一覧" });
  return user;
}

beforeEach(() => push.mockClear());

describe("期間と日時の表示", () => {
  test("期間の誤りは始まりが明日より前・終わりが始まりより前・31 日を超えるとき", () => {
    expect(leavePeriodErrors({ from: "", to: "" }, NOW)).toEqual({
      from: "始まりの日を選んでください。",
      to: "終わりの日を選んでください。",
    });
    expect(leavePeriodErrors({ from: "2026-11-05", to: "2026-11-06" }, NOW).from).toBe(
      "始まりの日は明日以降の日にしてください。",
    );
    expect(leavePeriodErrors({ from: "2026-11-06", to: "2026-11-06" }, NOW)).toEqual({});
    expect(leavePeriodErrors({ from: "2026-11-10", to: "2026-11-09" }, NOW).to).toBe(
      "終わりの日は始まりの日と同じか、それより後の日にしてください。",
    );
    expect(leavePeriodErrors({ from: "2026-11-10", to: "2026-12-10" }, NOW)).toEqual({});
    expect(leavePeriodErrors({ from: "2026-11-10", to: "2026-12-11" }, NOW).to).toBe(
      "お休みの期間は 31 日までにしてください。",
    );
  });

  test("日時は日本時間で「11/10（火）19:00〜19:30」、期間は「11/10（火）〜11/16（月）」", () => {
    expect(slotTimeLabel("2026-11-10T10:00:00Z", "2026-11-10T10:30:00Z")).toBe("11/10（火）19:00〜19:30");
    expect(slotTimeLabel("2026-11-15T00:00:00Z", "2026-11-15T00:30:00Z")).toBe("11/15（日）9:00〜9:30");
    expect(periodLabel({ from: "2026-11-10", to: "2026-11-16" })).toBe("11/10（火）〜11/16（月）");
  });
});

describe("本部のコーチのお休み", () => {
  test("コーチの様子の「このコーチで開く画面」の最後に「お休み」がある", () => {
    render(<CoachRelatedLinks coachId={COACH_ID} />);
    const links = within(screen.getByRole("navigation", { name: "このコーチで開く画面" })).getAllByRole("link");
    expect(links.at(-1)).toHaveTextContent("お休み");
    expect(links.at(-1)).toHaveAttribute("href", coachLeavePath(COACH_ID));
  });

  test("コーチがいなければ「見つかりません」を出し、入力欄は出さない", async () => {
    render(<CoachLeave api={apiOf()} coachId={undefined} now={() => NOW} />);
    expect(screen.getByText("このコーチは見つかりません。コーチの一覧から選び直してください。")).toBeInTheDocument();
    expect(screen.queryByLabelText("始まりの日")).not.toBeInTheDocument();
  });

  test("期間がまだ空なら「お休みにする」は押せず、誤った期間では API を呼ばずに欄の下へ誤りを出す", async () => {
    const api = apiOf();
    const user = userEvent.setup();
    render(<CoachLeave api={api} coachId={COACH_ID} now={() => NOW} />);
    await screen.findByText("山田 健");
    expect(screen.getByRole("button", { name: "お休みにする" })).toBeDisabled();
    expect(screen.getByText("期間を確かめると押せます。")).toBeInTheDocument();

    await fillPeriod(user, "2026-11-10", "2026-11-09");
    await user.click(screen.getByRole("button", { name: "期間の予約と枠を確かめる" }));

    expect(api.getCoachLeavePreview).not.toHaveBeenCalled();
    expect(screen.getByText("終わりの日は始まりの日と同じか、それより後の日にしてください。")).toBeInTheDocument();
    expect(screen.getByLabelText("終わりの日")).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByLabelText("終わりの日")).toHaveFocus();
  });

  test("確かめると期間の予約・予約のない枠・お知らせの見本・知らせを出す。行から予約の詳細へ移る", async () => {
    const api = apiOf();
    await openWithPreview(api);

    expect(api.getCoachLeavePreview).toHaveBeenCalledWith(COACH_ID, { from: "2026-11-10", to: "2026-11-16" });
    expect(screen.getByRole("heading", { name: "期間の予約（2 件）" })).toBeInTheDocument();
    const rows = within(screen.getByRole("list", { name: "期間の予約の一覧" })).getAllByRole("listitem");
    expect(rows[0]).toHaveTextContent("11/10（火）19:00〜19:30");
    expect(rows[0]).toHaveTextContent("生徒: 佐藤 花子・体験予約");
    expect(rows[0]).toHaveTextContent("確定済み");
    expect(rows[1]).toHaveTextContent("メニュー: フォアハンドの見直し");
    expect(rows[1]).toHaveTextContent("確定待ち");
    const slots = within(screen.getByRole("list", { name: "期間の予約のない枠の一覧" })).getAllByRole("listitem");
    expect(slots[0]).toHaveTextContent("11/15（日）9:00〜9:30");
    expect(slots[0]).toHaveTextContent("公開中");
    expect(screen.getByText("コーチのお休みで予約をキャンセルしました")).toBeInTheDocument();
    expect(screen.getByText(/キャンセルした予約は元に戻せません/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "お休みにする" })).toBeEnabled();
    expect(screen.getByText("テニス")).toBeInTheDocument();

    await userEvent.setup().click(within(rows[0] as HTMLElement).getByRole("button"));
    expect(push).toHaveBeenCalledWith(`/headquarters/bookings/detail?booking_id=${BOOKING_A}`);
  });

  test("一覧のあとに期間を変えると、一覧は残して「お休みにする」を押せなくする", async () => {
    const user = await openWithPreview(apiOf());
    await fillPeriod(user, "2026-11-11", "2026-11-16");
    expect(screen.getByRole("list", { name: "期間の予約の一覧" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "お休みにする" })).toBeDisabled();
    expect(screen.getByText("期間を変えました。「期間の予約と枠を確かめる」で読み直してください。")).toBeInTheDocument();
  });

  test("期間に予約も公開中の枠もなければ空の表示を出し、お休みにはできない", async () => {
    const api = apiOf({
      preview: {
        ok: true,
        data: { ...PREVIEW, bookings: [], open_slots: [], slot_count: 0, student_count: 0, notification_preview: undefined },
      },
    });
    const user = userEvent.setup();
    render(<CoachLeave api={api} coachId={COACH_ID} now={() => NOW} />);
    await screen.findByText("山田 健");
    await fillPeriod(user, "2026-11-10", "2026-11-16");
    await user.click(screen.getByRole("button", { name: "期間の予約と枠を確かめる" }));
    expect(
      await screen.findByText(/11\/10（火）〜11\/16（月） に、山田 健 コーチの予約と公開中の枠はありません。/),
    ).toBeInTheDocument();
  });

  test("一言が 200 文字を超えると数を出し、「お休みにする」でその欄へ移って送らない", async () => {
    const api = apiOf();
    const user = await openWithPreview(api);
    const note = screen.getByLabelText("生徒への一言（任意）");
    await act(async () => {
      await user.click(note);
      await user.paste("あ".repeat(201));
    });
    expect(screen.getByText("201 / 200")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "お休みにする" }));
    expect(screen.getByText("一言は 200 文字までにしてください。")).toBeInTheDocument();
    expect(note).toHaveFocus();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  test("確認で「やめる」なら何も送らず、「キャンセルしてお休みにする」で 1 回送って結果と次のリンクを出す", async () => {
    const api = apiOf();
    const user = await openWithPreview(api);
    await user.type(screen.getByLabelText("生徒への一言（任意）"), "ご迷惑をおかけします。");

    await user.click(screen.getByRole("button", { name: "お休みにする" }));
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getByRole("heading", { name: "山田 健 コーチをお休みにしますか？" })).toHaveFocus();
    expect(dialog).toHaveTextContent("予約 2 件をキャンセルして生徒 2 人にお知らせを送り、枠 3 件を取り下げます。");
    await user.click(within(dialog).getByRole("button", { name: "やめる" }));
    expect(api.startCoachLeave).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "お休みにする" })).toHaveFocus();

    await user.click(screen.getByRole("button", { name: "お休みにする" }));
    await user.click(screen.getByRole("button", { name: "キャンセルしてお休みにする" }));

    expect(api.startCoachLeave).toHaveBeenCalledTimes(1);
    expect(api.startCoachLeave).toHaveBeenCalledWith(COACH_ID, {
      from: "2026-11-10",
      to: "2026-11-16",
      note: "ご迷惑をおかけします。",
      expected_booking_ids: [BOOKING_A, BOOKING_B],
    });
    const status = await screen.findByText(
      "山田 健 コーチを 11/10（火）〜11/16（月）のお休みにしました。予約 2 件をキャンセルして生徒 2 人にお知らせを送り、枠 3 件を取り下げました。",
    );
    expect(status.closest('[role="status"]')).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("始まりの日")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "お休みにする" })).not.toBeInTheDocument();
    const rows = within(screen.getByRole("list", { name: "期間の予約の一覧" })).getAllByRole("listitem");
    expect(rows.every((row) => row.textContent?.includes("キャンセル済み"))).toBe(true);
    expect(within(screen.getByRole("list", { name: "期間の予約のない枠の一覧" })).getByText("終了")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "キャンセルされた予約を見る" }));
    expect(push).toHaveBeenCalledWith(`/headquarters/cancelled-bookings?coach_id=${COACH_ID}`);
  });

  test("確かめたあとに予約が変わって断られたら、何も変えていないことを出し、入れた値は残す", async () => {
    const api = apiOf({
      start: {
        ok: false,
        code: "leave_preview_outdated",
        message:
          "期間の予約が確かめたあとに変わったため、お休みにできませんでした。予約も枠も変えていません。「期間の予約と枠を確かめる」で読み直してから、もう一度「お休みにする」を押してください。",
      },
    });
    const user = await openWithPreview(api);
    await user.type(screen.getByLabelText("生徒への一言（任意）"), "よろしくお願いします。");
    await user.click(screen.getByRole("button", { name: "お休みにする" }));
    await user.click(screen.getByRole("button", { name: "キャンセルしてお休みにする" }));

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("期間の予約が確かめたあとに変わったため"),
    );
    expect(screen.getByLabelText("始まりの日")).toHaveValue("2026-11-10");
    expect(screen.getByLabelText("生徒への一言（任意）")).toHaveValue("よろしくお願いします。");
  });

  test("そのほかの失敗はサーバーの文言に「予約も枠も変えていません。」を添える", async () => {
    const api = apiOf({ start: { ok: false, message: "サーバーに接続できません。" } });
    const user = await openWithPreview(api);
    await user.click(screen.getByRole("button", { name: "お休みにする" }));
    await user.click(screen.getByRole("button", { name: "キャンセルしてお休みにする" }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("サーバーに接続できません。 予約も枠も変えていません。"),
    );
  });

  test("一覧が読めなければ欄の中に失敗を出し、期間は残す", async () => {
    const api = apiOf({ preview: { ok: false, message: "サーバーに接続できません。" } });
    const user = userEvent.setup();
    render(<CoachLeave api={api} coachId={COACH_ID} now={() => NOW} />);
    await screen.findByText("山田 健");
    await fillPeriod(user, "2026-11-10", "2026-11-16");
    await user.click(screen.getByRole("button", { name: "期間の予約と枠を確かめる" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("期間の予約を読み込めませんでした。");
    expect(screen.getByLabelText("始まりの日")).toHaveValue("2026-11-10");
  });
});
