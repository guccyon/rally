import { render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { CoachAcceptanceHistory } from "@/components/headquarters/CoachAcceptanceHistory";
import { CoachOverviewHome } from "@/components/headquarters/CoachOverviewHome";
import { formatDateTime } from "@/components/labels";

vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace: vi.fn() }) }));

const COACH_ID = "22222222-2222-4222-8222-000000000001";
const HQ_A = "44444444-4444-4444-8444-00000000000a";
const HQ_B = "44444444-4444-4444-8444-00000000000b";

const CHANGES: Schemas["CoachAcceptanceChangeList"] = {
  coach_id: COACH_ID,
  changes: [
    { acceptance_status: "accepting", changed_by: HQ_B, changed_at: "2026-10-30T01:00:00Z" },
    { acceptance_status: "suspended", changed_by: HQ_A, changed_at: "2026-10-28T09:30:00Z" },
  ],
};

function apiOf(result: Result<Schemas["CoachAcceptanceChangeList"]>) {
  return {
    listCoachAcceptanceChanges: vi.fn<HeadquartersApi["listCoachAcceptanceChanges"]>(async () => result),
  };
}

describe("本部のコーチの受付の記録", () => {
  test("読み込み中を出し、読めたら返った順（新しい順）に止めた・再開した・日時・操作した本部のアカウントを出す", async () => {
    const api = apiOf({ ok: true, data: CHANGES });
    render(<CoachAcceptanceHistory api={api} coachId={COACH_ID} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "受付の記録" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(api.listCoachAcceptanceChanges).toHaveBeenCalledTimes(1);
    expect(api.listCoachAcceptanceChanges).toHaveBeenCalledWith(COACH_ID);

    expect(within(table).getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual([
      "操作",
      "日時",
      "操作した本部のアカウント",
    ]);
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent))).toEqual([
      ["再開した", formatDateTime("2026-10-30T01:00:00Z"), HQ_B],
      ["止めた", formatDateTime("2026-10-28T09:30:00Z"), HQ_A],
    ]);
  });

  test("同じ時刻の記録も返った順のまま出す", async () => {
    const at = "2026-10-30T01:00:00Z";
    const data: Schemas["CoachAcceptanceChangeList"] = {
      coach_id: COACH_ID,
      changes: [
        { acceptance_status: "suspended", changed_by: HQ_A, changed_at: at },
        { acceptance_status: "accepting", changed_by: HQ_B, changed_at: at },
      ],
    };
    render(<CoachAcceptanceHistory api={apiOf({ ok: true, data })} coachId={COACH_ID} />);

    const table = await screen.findByRole("table", { name: "受付の記録" });
    const firstCells = within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("cell")[0]!.textContent);
    expect(firstCells).toEqual(["止めた", "再開した"]);
  });

  test("0 件なら「受付を止めたことはありません」を出す", async () => {
    render(
      <CoachAcceptanceHistory api={apiOf({ ok: true, data: { coach_id: COACH_ID, changes: [] } })} coachId={COACH_ID} />,
    );

    expect(await screen.findByText("受付を止めたことはありません")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("失敗したらその文言を出す", async () => {
    render(
      <CoachAcceptanceHistory
        api={apiOf({ ok: false, code: "forbidden", message: "権限がないためアクセスできません。" })}
        coachId={COACH_ID}
      />,
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "受付の記録を読めませんでした。権限がないためアクセスできません。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("受付を止める・再開する操作は出さない", async () => {
    render(<CoachAcceptanceHistory api={apiOf({ ok: true, data: CHANGES })} coachId={COACH_ID} />);

    await screen.findByRole("table", { name: "受付の記録" });
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  test("コーチの様子の画面に「受付の記録」の欄があり、コーチの ID がなければ欄を出さない", () => {
    const { unmount } = render(<CoachOverviewHome token="mock.headquarters.abc" coachId={COACH_ID} />);
    expect(screen.getByRole("region", { name: "受付の記録" })).toBeInTheDocument();
    unmount();

    render(<CoachOverviewHome token="mock.headquarters.abc" coachId={undefined} />);
    expect(screen.queryByRole("region", { name: "受付の記録" })).not.toBeInTheDocument();
  });
});
