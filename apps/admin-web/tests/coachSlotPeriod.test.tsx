import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { CoachApi } from "@/api/coach";
import { PublishSlotForm } from "@/components/coach/PublishSlotForm";

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const HOUR = 60 * 60 * 1000;
const at = (offsetHours: number) => new Date(Date.now() + offsetHours * HOUR).toISOString();

function coachSlot(id: string, sport: Schemas["Sport"], startsIn: number): Schemas["CoachSlot"] {
  return {
    slot: {
      slot_id: id,
      coach_id: COACH_ID,
      sport,
      starts_at: at(startsIn),
      ends_at: at(startsIn + 1),
      status: "open",
      capacity: 1,
      remaining_seats: 1,
      lesson_menu: null,
    },
    has_active_booking: false,
  };
}

const TENNIS = coachSlot("c0000000-0000-4000-8000-000000000001", "tennis", 24);
const SOCCER = coachSlot("c0000000-0000-4000-8000-000000000002", "soccer", 200);
const PERIOD = { from: "2026-11-09", to: "2026-11-15" };

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

function renderForm(api: Partial<Pick<CoachApi, "publishSlot" | "withdrawMySlot">> & Pick<CoachApi, "listMyCoachSlots">) {
  render(
    <PublishSlotForm
      api={{ listMyLessonMenus: noMenus(), publishSlot: vi.fn(), withdrawMySlot: vi.fn(), ...api }}
    />,
  );
}

async function choosePeriod(user: ReturnType<typeof userEvent.setup>, from: string, to: string) {
  await user.type(screen.getByLabelText("開始日"), from);
  await user.type(screen.getByLabelText("終了日"), to);
  await user.click(screen.getByRole("button", { name: "この期間で見る" }));
}

describe("自分の枠の一覧の期間", () => {
  test("期間を付けずに開き、「この期間で見る」で from・to に渡して読み直し、「期間を外す」ですべての枠に戻す", async () => {
    const user = userEvent.setup();
    const listMyCoachSlots = listOf([TENNIS, SOCCER], [SOCCER], [TENNIS, SOCCER]);
    renderForm({ listMyCoachSlots });

    await screen.findByRole("row", { name: /テニス/ });
    expect(listMyCoachSlots).toHaveBeenNthCalledWith(1, {});
    expect(screen.queryByRole("button", { name: "期間を外す" })).not.toBeInTheDocument();

    await choosePeriod(user, PERIOD.from, PERIOD.to);
    expect(listMyCoachSlots).toHaveBeenNthCalledWith(2, PERIOD);
    await screen.findByRole("row", { name: /サッカー/ });
    expect(screen.queryByRole("row", { name: /テニス/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "期間を外す" }));
    expect(listMyCoachSlots).toHaveBeenNthCalledWith(3, {});
    expect(await screen.findByRole("row", { name: /テニス/ })).toBeInTheDocument();
    expect(screen.getByLabelText("開始日")).toHaveValue("");
    expect(screen.getByLabelText("終了日")).toHaveValue("");
    expect(screen.queryByRole("button", { name: "期間を外す" })).not.toBeInTheDocument();
  });

  test("期間を確定したあとに公開したら、確定した期間で取り直す（欄を書き換えただけの期間は使わない）", async () => {
    const user = userEvent.setup();
    const published = coachSlot("c0000000-0000-4000-8000-000000000003", "baseball", 210);
    const publishSlot = vi.fn<CoachApi["publishSlot"]>(async () => ({ ok: true, data: published.slot }));
    const listMyCoachSlots = listOf([TENNIS, SOCCER], [SOCCER], [SOCCER, published]);
    renderForm({ listMyCoachSlots, publishSlot });

    await screen.findByRole("row", { name: /テニス/ });
    await choosePeriod(user, PERIOD.from, PERIOD.to);
    await screen.findByRole("button", { name: "期間を外す" });
    // 確定していない書き換え。
    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-11-20");

    await user.type(screen.getByLabelText("開始"), "2026-11-10T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-10T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    expect(await screen.findByRole("status")).toHaveTextContent("枠を公開しました。");
    expect(listMyCoachSlots).toHaveBeenCalledTimes(3);
    expect(listMyCoachSlots).toHaveBeenNthCalledWith(3, PERIOD);
    expect(await screen.findByRole("row", { name: /野球/ })).toBeInTheDocument();
  });

  test("期間を付けずに開いたまま公開したら、今までどおり期間を付けずに取り直す", async () => {
    const user = userEvent.setup();
    const publishSlot = vi.fn<CoachApi["publishSlot"]>(async () => ({ ok: true, data: TENNIS.slot }));
    const listMyCoachSlots = listOf([], [TENNIS]);
    renderForm({ listMyCoachSlots, publishSlot });

    await screen.findByText("まだ枠を公開していません。");
    await user.type(screen.getByLabelText("開始"), "2026-11-10T10:00");
    await user.type(screen.getByLabelText("終了"), "2026-11-10T11:00");
    await user.click(screen.getByRole("button", { name: "枠を公開" }));

    await screen.findByRole("row", { name: /テニス/ });
    expect(listMyCoachSlots).toHaveBeenNthCalledWith(2, {});
  });

  test("取り下げが 409 で一覧を取り直すときも、確定した期間で読む", async () => {
    const user = userEvent.setup();
    const listMyCoachSlots = listOf([TENNIS], [TENNIS], [TENNIS]);
    const withdrawMySlot = vi.fn<CoachApi["withdrawMySlot"]>(async () => ({
      ok: false,
      code: "slot_has_active_booking",
      message: "予約が入っている枠は取り下げられません。",
    }));
    renderForm({ listMyCoachSlots, withdrawMySlot });

    await screen.findByRole("row", { name: /テニス/ });
    await choosePeriod(user, PERIOD.from, PERIOD.to);
    await screen.findByRole("button", { name: "期間を外す" });
    const row = await screen.findByRole("row", { name: /テニス/ });
    await user.click(within(row).getByRole("button", { name: "取り下げる" }));
    await user.click(within(row).getByRole("button", { name: "取り下げを確定" }));

    expect(await screen.findByRole("alert")).toBeInTheDocument();
    await vi.waitFor(() => expect(listMyCoachSlots).toHaveBeenCalledTimes(3));
    expect(listMyCoachSlots).toHaveBeenNthCalledWith(3, PERIOD);
  });

  test.each([
    ["", "", "開始日と終了日を選んでください。"],
    ["2026-11-15", "2026-11-09", "終了日は開始日と同じか、それより後の日付にしてください。"],
    ["2026-11-01", "2026-12-02", "期間は 31 日までにしてください。"],
  ])("誤った期間（%s〜%s）は送らずに理由を出す", async (from, to, message) => {
    const user = userEvent.setup();
    const listMyCoachSlots = listOf([TENNIS]);
    renderForm({ listMyCoachSlots });

    await screen.findByRole("row", { name: /テニス/ });
    if (from !== "") await user.type(screen.getByLabelText("開始日"), from);
    if (to !== "") await user.type(screen.getByLabelText("終了日"), to);
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));

    expect(screen.getByRole("alert")).toHaveTextContent(message);
    expect(listMyCoachSlots).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("row", { name: /テニス/ })).toBeInTheDocument();
  });

  test("期間を付けたときの API の 422 は、期間の直し方を添えた理由を出す", async () => {
    const user = userEvent.setup();
    const listMyCoachSlots = vi.fn<CoachApi["listMyCoachSlots"]>(async (period = {}) =>
      period.from === undefined
        ? { ok: true, data: { slots: [TENNIS] } }
        : { ok: false, code: "validation_failed", message: "入力が正しくありません。" },
    );
    renderForm({ listMyCoachSlots });

    await screen.findByRole("row", { name: /テニス/ });
    await choosePeriod(user, PERIOD.from, PERIOD.to);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "期間の指定が正しくありません。期間は 1〜31 日で、終了日は開始日と同じかそれより後の日付にしてください。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("期間に枠がなければ、期間を含めて空と出す", async () => {
    const user = userEvent.setup();
    renderForm({ listMyCoachSlots: listOf([TENNIS], []) });

    await screen.findByRole("row", { name: /テニス/ });
    await choosePeriod(user, PERIOD.from, PERIOD.to);

    expect(
      await screen.findByText("2026年11月9日（月） 〜 2026年11月15日（日）に始まる枠はありません。"),
    ).toBeInTheDocument();
    expect(screen.queryByText("まだ枠を公開していません。")).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});
