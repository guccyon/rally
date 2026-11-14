import { render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

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
  // 記録の日時は今年なら年を省くので、今日を固定する（Date だけ。タイマーは本物のまま）。
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2026-11-08T03:00:00Z"));
});

afterEach(() => {
  vi.useRealTimers();
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

const CONFIRMED: Schemas["BookingStatusChange"] = { status: "confirmed", changed_at: "2026-10-20T01:00:00Z" };

/** 生徒が開始の 17 時間前に自分でキャンセルし、枠を 1 回使った記録。 */
const STUDENT_CANCEL: Schemas["BookingStatusChange"] = {
  status: "cancelled",
  changed_at: "2026-10-26T08:00:00Z",
  cancelled_by: "student",
  consumes_quota: true,
  hours_before_start: 17,
  cancel_reason: null,
  coach_leave: null,
};

const COACH_LEAVE: Schemas["CoachLeave"] = {
  coach_leave_id: "77777777-7777-4777-8777-000000000001",
  coach_id: COACH_ID,
  from: "2026-11-10",
  to: "2026-11-16",
  note: "体調を崩したため、この期間のレッスンをお休みします。\nほかのコーチでの予約をお願いします。",
  created_by: "44444444-4444-4444-8444-000000000001",
  created_by_display_name: "佐々木 陽子",
  created_at: "2026-11-05T01:32:00Z",
};

/** 本部がコーチのお休みでキャンセルした記録。 */
const LEAVE_CANCEL: Schemas["BookingStatusChange"] = {
  status: "cancelled",
  changed_at: "2026-11-05T01:32:00Z",
  cancelled_by: "headquarters",
  consumes_quota: false,
  hours_before_start: 175,
  cancel_reason: "coach_leave",
  coach_leave: COACH_LEAVE,
};

/** 経緯を残し始める前のキャンセル（時間だけがある）。 */
const LEGACY_CANCEL: Schemas["BookingStatusChange"] = {
  status: "cancelled",
  changed_at: "2026-10-22T09:30:00Z",
  cancelled_by: null,
  consumes_quota: null,
  hours_before_start: 103,
  cancel_reason: null,
  coach_leave: null,
};

const DETAIL: Schemas["HeadquartersBookingDetail"] = {
  booking: BOOKING,
  slot: SLOT,
  status_changes: [CONFIRMED, STUDENT_CANCEL],
  coach_display_name: "鈴木 コーチ",
  student_display_name: "山田 花子",
};

function detailOf(detail: Schemas["HeadquartersBookingDetail"]) {
  return vi.fn<HeadquartersApi["getBooking"]>(async () => ({ ok: true, data: detail }));
}

/** 説明の並び（dt と dd）を [見出し, 値] の組にする。記録のカードの中の並びは除く（上の予約の項目だけ）。 */
function termsOf() {
  const outside = (element: HTMLElement) => element.closest("article") === null;
  const terms = screen.getAllByRole("term").filter(outside).map((term) => term.textContent);
  const values = screen.getAllByRole("definition").filter(outside).map((value) => value.textContent);
  return terms.map((term, index) => [term, values[index]]);
}

/** 記録のカードの aria-label の並び。 */
function cardsOf() {
  return screen.getAllByRole("article").map((card) => card.getAttribute("aria-label"));
}

/** カードの中の、名前（aria-label）の付いたラベルと値の並びを [見出し, 値] の組にする。 */
function listOf(card: HTMLElement, name: string) {
  const list = card.querySelector(`dl[aria-label="${name}"]`);
  if (!list) return undefined;
  return Array.from(list.querySelectorAll("dt")).map((term) => [term.textContent, term.nextElementSibling?.textContent]);
}

function cardNamed(name: string) {
  return screen.getByRole("article", { name });
}

describe("本部の予約の詳細", () => {
  test("読み込み中を出し、読めたら生徒・コーチ・メニュー・枠の時刻・種類・今の状態と、状態が変わった記録を API の順で出す", async () => {
    const getBooking = detailOf(DETAIL);
    render(<BookingDetail api={{ getBooking }} bookingId={BOOKING_ID} />);

    expect(screen.getByRole("status")).toHaveTextContent("予約の詳細を読み込んでいます…");
    await screen.findAllByRole("article");
    expect(screen.queryByText("予約の詳細を読み込んでいます…")).not.toBeInTheDocument();
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
    expect(cardsOf()).toEqual(["10/20（火）10:00 確定済み", "10/26（月）17:00 キャンセル済み"]);
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("今の状態と記録の状態は、予約の一覧と同じ状態のタグ（色の点と字）で出す", async () => {
    render(<BookingDetail api={{ getBooking: detailOf(DETAIL) }} bookingId={BOOKING_ID} />);

    await screen.findAllByRole("article");
    const current = screen.getAllByRole("definition")[5]!;
    expect(within(within(current).getByText("キャンセル済み")).getByTestId("status-dot")).toHaveClass("bg-border-strong");
    const confirmed = cardNamed("10/20（火）10:00 確定済み");
    expect(within(within(confirmed).getByText("確定済み")).getByTestId("status-dot")).toHaveClass("bg-success");
    const cancelled = cardNamed("10/26（月）17:00 キャンセル済み");
    expect(within(within(cancelled).getByText("キャンセル済み")).getByTestId("status-dot")).toHaveClass("bg-border-strong");
  });

  test("確定だけの予約は、確定のカード 1 枚に「予約が確定しました。」を出し、経緯の並びを出さない", async () => {
    render(
      <BookingDetail
        api={{ getBooking: detailOf({ ...DETAIL, booking: { ...BOOKING, status: "confirmed" }, status_changes: [CONFIRMED] }) }}
        bookingId={BOOKING_ID}
      />,
    );

    const card = await screen.findByRole("article", { name: "10/20（火）10:00 確定済み" });
    expect(cardsOf()).toHaveLength(1);
    expect(card).toHaveTextContent("予約が確定しました。");
    expect(listOf(card, "キャンセルの経緯")).toBeUndefined();
  });

  test("開始まで（hours_before_start）が載っていないキャンセルは、枠の時刻とキャンセルした日時から出し、説明文と食い違わない", async () => {
    const { hours_before_start: _omitted, ...rest } = LEGACY_CANCEL;
    render(
      <BookingDetail
        api={{ getBooking: detailOf({ ...DETAIL, status_changes: [CONFIRMED, rest as Schemas["BookingStatusChange"]] }) }}
        bookingId={BOOKING_ID}
      />,
    );

    const card = await screen.findByRole("article", { name: "10/22（木）18:30 キャンセル済み" });
    expect(listOf(card, "キャンセルの経緯")?.[1]).toEqual(["開始まで", expect.stringMatching(/^開始の \d+ 時間/)]);
    expect(within(card).getByRole("status")).toHaveTextContent("開始の何時間前かは、枠の時刻とキャンセルした日時から出しています");
  });

  test("生徒のキャンセル（枠を使った）は、キャンセルした人・開始まで・ビデオ通話の枠を文で出し、理由の行を出さない", async () => {
    render(<BookingDetail api={{ getBooking: detailOf(DETAIL) }} bookingId={BOOKING_ID} />);

    const card = await screen.findByRole("article", { name: "10/26（月）17:00 キャンセル済み" });
    expect(listOf(card, "キャンセルの経緯")).toEqual([
      ["キャンセルした人", "生徒（自分でキャンセル）"],
      ["開始まで", "開始の 17 時間前"],
      ["ビデオ通話の枠", "1 回使いました（キャンセル規定: 開始 24 時間前より後のキャンセル）"],
    ]);
    expect(within(card).queryByRole("status")).not.toBeInTheDocument();
    expect(card).not.toHaveTextContent(/student|true|false|null/);
  });

  test("生徒の 24 時間前より前のキャンセルは、枠を使っていないと出し、日と時間を添える", async () => {
    const change = { ...STUDENT_CANCEL, consumes_quota: false, hours_before_start: 50 };
    render(<BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [CONFIRMED, change] }) }} bookingId={BOOKING_ID} />);

    const card = await screen.findByRole("article", { name: "10/26（月）17:00 キャンセル済み" });
    expect(listOf(card, "キャンセルの経緯")).toEqual([
      ["キャンセルした人", "生徒（自分でキャンセル）"],
      ["開始まで", "開始の 50 時間前（2 日 2 時間前）"],
      ["ビデオ通話の枠", "使っていません（開始 24 時間前までのキャンセル）"],
    ]);
  });

  test("コーチのお休みのキャンセルは、理由と、もとになったお休みの期間・生徒への一言・お休みにした本部と時刻を出す", async () => {
    render(
      <BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [CONFIRMED, LEAVE_CANCEL] }) }} bookingId={BOOKING_ID} />,
    );

    const card = await screen.findByRole("article", { name: "11/5（木）10:32 キャンセル済み" });
    expect(listOf(card, "キャンセルの経緯")).toEqual([
      ["キャンセルした人", "本部（コーチのお休み）"],
      ["開始まで", "開始の 175 時間前（7 日 7 時間前）"],
      ["ビデオ通話の枠", "使っていません（コーチのお休みのキャンセルは回数に数えません）"],
      ["理由", "コーチのお休み"],
    ]);
    expect(within(card).getByText("もとになったコーチのお休み", { selector: "p" })).toBeInTheDocument();
    expect(listOf(card, "もとになったコーチのお休み")).toEqual([
      ["お休みの期間", "11/10（火）〜11/16（月）"],
      ["生徒への一言", COACH_LEAVE.note],
      ["お休みにした本部", "佐々木 陽子・11/5（木）10:32"],
    ]);
    // ID・コードは出さない
    expect(card).not.toHaveTextContent(COACH_LEAVE.created_by);
    expect(card).not.toHaveTextContent("coach_leave");
  });

  test("お休みの一言がない・本部の名前が引けないときは、それぞれの言葉で出す", async () => {
    const change = { ...LEAVE_CANCEL, coach_leave: { ...COACH_LEAVE, note: "", created_by_display_name: undefined } };
    render(<BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [change] }) }} bookingId={BOOKING_ID} />);

    const card = await screen.findByRole("article", { name: "11/5（木）10:32 キャンセル済み" });
    expect(listOf(card, "もとになったコーチのお休み")).toEqual([
      ["お休みの期間", "11/10（火）〜11/16（月）"],
      ["生徒への一言", "（一言はありません）"],
      ["お休みにした本部", "（名前を表示できません）・11/5（木）10:32"],
    ]);
  });

  test("理由がお休みなのにお休みの記録がないときは、理由だけ出し、お休みの区切りの代わりに 1 行の注意を出す", async () => {
    const change = { ...LEAVE_CANCEL, coach_leave: null };
    render(<BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [change] }) }} bookingId={BOOKING_ID} />);

    const card = await screen.findByRole("article", { name: "11/5（木）10:32 キャンセル済み" });
    expect(listOf(card, "キャンセルの経緯")).toContainEqual(["理由", "コーチのお休み"]);
    expect(listOf(card, "もとになったコーチのお休み")).toBeUndefined();
    expect(card).toHaveTextContent("お休みの記録を読み込めませんでした。開発の担当に知らせてください。");
  });

  test("本部の理由のないキャンセル・コーチのキャンセルは、理由の決まりに合わせて書き分ける", async () => {
    const hq = { ...STUDENT_CANCEL, cancelled_by: "headquarters" as const, consumes_quota: false, hours_before_start: 0 };
    const coach = { ...STUDENT_CANCEL, changed_at: "2026-10-27T03:00:00Z", cancelled_by: "coach" as const, consumes_quota: false, hours_before_start: -2 };
    render(<BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [hq, coach] }) }} bookingId={BOOKING_ID} />);

    const hqCard = await screen.findByRole("article", { name: "10/26（月）17:00 キャンセル済み" });
    expect(listOf(hqCard, "キャンセルの経緯")).toEqual([
      ["キャンセルした人", "本部"],
      ["開始まで", "開始の 1 時間以内"],
      ["ビデオ通話の枠", "使っていません"],
      ["理由", "記録がありません（本部のキャンセルで、理由が残っていません）"],
    ]);
    expect(listOf(cardNamed("10/27（火）12:00 キャンセル済み"), "キャンセルの経緯")).toEqual([
      ["キャンセルした人", "コーチ"],
      ["開始まで", "開始の 2 時間後"],
      ["ビデオ通話の枠", "使っていません"],
    ]);
  });

  test("記録のない古いキャンセルは、値ごとに「記録がありません」と書き、カードの中に知らせを 1 つ出す", async () => {
    render(
      <BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [CONFIRMED, LEGACY_CANCEL] }) }} bookingId={BOOKING_ID} />,
    );

    const card = await screen.findByRole("article", { name: "10/22（木）18:30 キャンセル済み" });
    expect(listOf(card, "キャンセルの経緯")).toEqual([
      ["キャンセルした人", "記録がありません"],
      ["開始まで", "開始の 103 時間前（4 日 7 時間前）"],
      ["ビデオ通話の枠", "記録がありません"],
      ["理由", "記録がありません"],
    ]);
    expect(within(card).getByRole("status")).toHaveTextContent(
      "この予約は、キャンセルの経緯を残し始める前にキャンセルされました。",
    );
    expect(within(cardNamed("10/20（火）10:00 確定済み")).queryByRole("status")).not.toBeInTheDocument();
  });

  test("経緯の欄が載っていないキャンセル（欄ごと省いた）も、記録のない古いキャンセルと同じに出す", async () => {
    const change: Schemas["BookingStatusChange"] = { status: "cancelled", changed_at: "2026-10-22T09:30:00Z" };
    render(<BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [change] }) }} bookingId={BOOKING_ID} />);

    const card = await screen.findByRole("article", { name: "10/22（木）18:30 キャンセル済み" });
    expect(listOf(card, "キャンセルの経緯")).toEqual([
      ["キャンセルした人", "記録がありません"],
      ["開始まで", "記録がありません"],
      ["ビデオ通話の枠", "記録がありません"],
      ["理由", "記録がありません"],
    ]);
    expect(within(card).getByRole("status")).toBeInTheDocument();
  });

  test("片方だけ記録がないキャンセルは、その行を「記録がありません」にし、知らせは出さない", async () => {
    const change = { ...STUDENT_CANCEL, consumes_quota: null };
    render(<BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [change] }) }} bookingId={BOOKING_ID} />);

    const card = await screen.findByRole("article", { name: "10/26（月）17:00 キャンセル済み" });
    expect(listOf(card, "キャンセルの経緯")).toContainEqual(["ビデオ通話の枠", "記録がありません"]);
    expect(within(card).queryByRole("status")).not.toBeInTheDocument();
  });

  test("今年でない記録の日時には年を足す", async () => {
    const change = { ...CONFIRMED, changed_at: "2025-12-01T00:00:00Z" };
    render(<BookingDetail api={{ getBooking: detailOf({ ...DETAIL, status_changes: [change] }) }} bookingId={BOOKING_ID} />);

    expect(await screen.findByRole("article", { name: "2025/12/1（月）9:00 確定済み" })).toBeInTheDocument();
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

    await screen.findByText("まだ確定もキャンセルもしていない予約です。コーチが確定すると、ここに記録が出ます。");
    expect(termsOf()).toEqual([
      ["生徒", "（名前を表示できません）"],
      ["コーチ", "（名前を表示できません）"],
      ["レッスンメニュー", "—"],
      ["枠の時刻", "2026/10/27 23:30:00 〜 2026/10/28 0:30:00"],
      ["種類", "—"],
      ["今の状態", "確定待ち"],
      ["予約した日時", "2026/10/20 9:00:00"],
    ]);
    expect(screen.queryByRole("article")).not.toBeInTheDocument();
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
    expect(screen.queryByRole("article")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("予約の詳細を読み込んでいます…")).not.toBeInTheDocument());
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
