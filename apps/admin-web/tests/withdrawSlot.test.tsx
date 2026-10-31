import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { PublishSlotForm } from "@/components/coach/PublishSlotForm";

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const HOUR = 60 * 60 * 1000;
const at = (offsetHours: number) => new Date(Date.now() + offsetHours * HOUR).toISOString();

function coachSlot(
  id: string,
  sport: Schemas["Sport"],
  startsIn: number,
  status: Schemas["SlotStatus"],
  hasActiveBooking: boolean,
  capacity = 1,
): Schemas["CoachSlot"] {
  return {
    slot: {
      slot_id: id,
      coach_id: COACH_ID,
      sport,
      starts_at: at(startsIn),
      ends_at: at(startsIn + 1),
      status,
      capacity,
      remaining_seats: hasActiveBooking ? capacity - 1 : capacity,
      lesson_menu: { lesson_menu_id: "a0000000-0000-4000-8000-000000000001", name: "フォーム改善", duration_minutes: 60 },
    },
    has_active_booking: hasActiveBooking,
  };
}

// これからの枠（終了が今より後）。取り下げられるのは予約のない公開中のまだ始まっていない枠（テニス・水泳）だけ。
const WITHDRAWABLE = coachSlot("c0000000-0000-4000-8000-000000000001", "tennis", 24, "open", false);
const GROUP_WITHDRAWABLE = coachSlot("c0000000-0000-4000-8000-000000000002", "swimming", 30, "open", false, 4);
const OPEN_BOOKED = coachSlot("c0000000-0000-4000-8000-000000000003", "soccer", 36, "open", true, 4);
const BOOKED = coachSlot("c0000000-0000-4000-8000-000000000004", "golf", 48, "booked", true);
const CLOSED = coachSlot("c0000000-0000-4000-8000-000000000005", "baseball", 60, "closed", false);
const STARTED = coachSlot("c0000000-0000-4000-8000-000000000006", "basketball", -0.5, "open", false);
// API の並び（開始時刻の古い順）。
const SLOTS = [STARTED, WITHDRAWABLE, GROUP_WITHDRAWABLE, OPEN_BOOKED, BOOKED, CLOSED];

function listOf(...lists: Schemas["CoachSlot"][][]) {
  const fn = vi.fn<CoachApi["listMyCoachSlots"]>();
  for (const slots of lists) fn.mockResolvedValueOnce({ ok: true, data: { slots } });
  return fn;
}

function noMenus() {
  return vi.fn<CoachApi["listMyLessonMenus"]>(async () => ({
    ok: true,
    data: { coach_id: COACH_ID, lesson_menus: [] },
  }));
}

function renderForm(
  withdrawMySlot: CoachApi["withdrawMySlot"],
  listMyCoachSlots: CoachApi["listMyCoachSlots"] = listOf(SLOTS),
) {
  render(
    <PublishSlotForm
      api={{ listMyLessonMenus: noMenus(), publishSlot: vi.fn(), listMyCoachSlots, withdrawMySlot }}
    />,
  );
}

async function rowOf(sport: string) {
  const table = await screen.findByRole("table", { name: "これからの枠" });
  return within(table)
    .getAllByRole("row")
    .find((row) => within(row).queryAllByRole("cell")[0]?.textContent === sport)!;
}

describe("自分の枠の取り下げ", () => {
  test("予約の入っていない公開中のまだ始まっていない枠の行にだけ「取り下げる」を出す", async () => {
    renderForm(vi.fn());

    for (const sport of ["テニス", "水泳"]) {
      expect(within(await rowOf(sport)).getByRole("button", { name: "取り下げる" })).toBeInTheDocument();
    }
    // 予約が入っている（公開中のグループの枠も、予約済みも）・閉じた・始まった枠には出さない。
    for (const sport of ["サッカー", "ゴルフ", "野球", "バスケットボール"]) {
      expect(within(await rowOf(sport)).queryByRole("button", { name: "取り下げる" })).not.toBeInTheDocument();
    }
  });

  test("押したら行の中で確かめ、「やめる」なら送らない", async () => {
    const user = userEvent.setup();
    const withdrawMySlot = vi.fn<CoachApi["withdrawMySlot"]>();
    const confirmSpy = vi.spyOn(window, "confirm");
    renderForm(withdrawMySlot);

    const row = await rowOf("テニス");
    await user.click(within(row).getByRole("button", { name: "取り下げる" }));

    const group = within(row).getByRole("group", { name: "枠の取り下げの確認" });
    expect(group).toHaveTextContent("この枠を取り下げますか？");
    expect(withdrawMySlot).not.toHaveBeenCalled();
    expect(confirmSpy).not.toHaveBeenCalled();

    await user.click(within(group).getByRole("button", { name: "やめる" }));
    expect(within(row).queryByRole("group")).not.toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "取り下げる" })).toBeInTheDocument();
    expect(withdrawMySlot).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  test("確定したらその枠を送り、行の状態を「終了」にしてボタンを消す。メニューはそのまま", async () => {
    const user = userEvent.setup();
    const listMyCoachSlots = listOf(SLOTS);
    const withdrawMySlot = vi.fn<CoachApi["withdrawMySlot"]>(async () => ({
      ok: true,
      // 応答の Slot には lesson_menu が載らない（契約）。
      data: { ...WITHDRAWABLE.slot, status: "closed", lesson_menu: undefined },
    }));
    renderForm(withdrawMySlot, listMyCoachSlots);

    const row = await rowOf("テニス");
    expect(within(row).getAllByRole("cell")[3]).toHaveTextContent("公開中");
    await user.click(within(row).getByRole("button", { name: "取り下げる" }));
    await user.click(within(row).getByRole("button", { name: "取り下げを確定" }));

    expect(withdrawMySlot).toHaveBeenCalledWith(WITHDRAWABLE.slot.slot_id);
    expect(await screen.findByRole("status")).toHaveTextContent("枠を取り下げました。");
    const updated = await rowOf("テニス");
    expect(within(updated).getAllByRole("cell")[3]).toHaveTextContent("終了");
    expect(within(updated).getAllByRole("cell")[1]).toHaveTextContent("フォーム改善");
    expect(within(updated).queryByRole("button", { name: "取り下げる" })).not.toBeInTheDocument();
    // 他の枠はそのまま。一覧は取り直さない。
    expect(within(await rowOf("水泳")).getByRole("button", { name: "取り下げる" })).toBeInTheDocument();
    expect(listMyCoachSlots).toHaveBeenCalledTimes(1);
  });

  test("409 はサーバーの ErrorCode の文言を出し、一覧を取り直す", async () => {
    const user = userEvent.setup();
    // 読み込んだ後に予約が入った。
    const nowBooked = { ...WITHDRAWABLE, has_active_booking: true };
    const listMyCoachSlots = listOf(SLOTS, [STARTED, nowBooked, GROUP_WITHDRAWABLE, OPEN_BOOKED, BOOKED, CLOSED]);
    const withdrawMySlot = vi.fn<CoachApi["withdrawMySlot"]>(async () => ({
      ok: false,
      code: "slot_has_active_booking",
      message: "予約が入っている枠は取り下げられません。",
    }));
    renderForm(withdrawMySlot, listMyCoachSlots);

    const row = await rowOf("テニス");
    await user.click(within(row).getByRole("button", { name: "取り下げる" }));
    await user.click(within(row).getByRole("button", { name: "取り下げを確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("予約が入っている枠は取り下げられません。");
    expect(listMyCoachSlots).toHaveBeenCalledTimes(2);
    const updated = await rowOf("テニス");
    expect(within(updated).getAllByRole("cell")[4]).toHaveTextContent("予約あり");
    expect(within(updated).queryByRole("button", { name: "取り下げる" })).not.toBeInTheDocument();
  });

  test("終わった枠の一覧には取り下げの列を出さない", async () => {
    const past = coachSlot("c0000000-0000-4000-8000-000000000007", "tennis", -48, "open", false);
    renderForm(vi.fn(), listOf([past, WITHDRAWABLE]));

    const table = await screen.findByRole("table", { name: "終わった枠" });
    expect(within(table).queryByRole("columnheader", { name: "取り下げ" })).not.toBeInTheDocument();
    expect(within(table).queryByRole("button")).not.toBeInTheDocument();
  });
});
