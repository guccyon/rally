import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { SlotSupplyHome } from "@/components/headquarters/SlotSupplyHome";
import { SlotSupplyReport } from "@/components/headquarters/SlotSupplyReport";
import { formatDateTime } from "@/components/labels";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const REPORT: Schemas["SlotSupplyReport"] = {
  window_start: "2026-09-27T03:00:00Z",
  window_end: "2026-10-27T03:00:00Z",
  sports: [
    { sport: "tennis", status: "insufficient", published_slots: 10, booking_students: 8 },
    { sport: "golf", status: "sufficient", published_slots: 0, booking_students: 0 },
    { sport: "baseball", status: "sufficient", published_slots: 12, booking_students: 3 },
    { sport: "soccer", status: "insufficient", published_slots: 5, booking_students: 4 },
    { sport: "basketball", status: "sufficient", published_slots: 7, booking_students: 1 },
    { sport: "swimming", status: "insufficient", published_slots: 2, booking_students: 6 },
  ],
};

describe("スポーツ別の予約枠の需給", () => {
  test("スポーツごとに判定と判定に使った数を一覧にし、集計の期間を出す", async () => {
    const getSlotSupply = vi.fn<HeadquartersApi["getSlotSupply"]>(async () => ({
      ok: true,
      data: REPORT,
    }));
    render(<SlotSupplyReport api={{ getSlotSupply }} />);

    const tennis = await screen.findByRole("row", { name: /テニス/ });
    expect(within(tennis).getAllByRole("cell").map((cell) => cell.textContent)).toEqual([
      "テニス",
      "足りていない",
      "10",
      "8",
    ]);
    const golf = screen.getByRole("row", { name: /ゴルフ/ });
    expect(within(golf).getAllByRole("cell").map((cell) => cell.textContent)).toEqual([
      "ゴルフ",
      "足りている",
      "0",
      "0",
    ]);
    const swimming = screen.getByRole("row", { name: /水泳/ });
    expect(within(swimming).getAllByRole("cell").map((cell) => cell.textContent)).toEqual([
      "水泳",
      "足りていない",
      "2",
      "6",
    ]);
    // 契約の順（Sport の enum の順）のまま出す。見出しの行を除いて 6 行
    expect(screen.getAllByRole("row").slice(1).map((row) => row.firstChild?.textContent)).toEqual([
      "テニス",
      "ゴルフ",
      "野球",
      "サッカー",
      "バスケットボール",
      "水泳",
    ]);
    expect(screen.getByText(/集計の期間/)).toHaveTextContent(
      `${formatDateTime(REPORT.window_start)} 〜 ${formatDateTime(REPORT.window_end)}`,
    );
    expect(getSlotSupply).toHaveBeenCalledTimes(1);
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const getSlotSupply = vi.fn<HeadquartersApi["getSlotSupply"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<SlotSupplyReport api={{ getSlotSupply }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("再集計で API を呼び直す", async () => {
    const getSlotSupply = vi.fn<HeadquartersApi["getSlotSupply"]>(async () => ({
      ok: true,
      data: REPORT,
    }));
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply }} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "再集計" }));

    await screen.findByRole("table");
    expect(getSlotSupply).toHaveBeenCalledTimes(2);
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">{(session) => <SlotSupplyHome token={session.token} />}</RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "スポーツ別の予約枠の需給" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
