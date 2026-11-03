import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";

import type { Result } from "@/api/call";
import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { CoachAcceptanceHistory } from "@/components/headquarters/CoachAcceptanceHistory";
import { CoachOverviewHome } from "@/components/headquarters/CoachOverviewHome";
import { formatDateTime, formatDay } from "@/components/labels";

vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace: vi.fn() }) }));

const COACH_ID = "22222222-2222-4222-8222-000000000001";
const HQ_A = "44444444-4444-4444-8444-00000000000a";
const HQ_B = "44444444-4444-4444-8444-00000000000b";

const CHANGES: Schemas["CoachAcceptanceChangeList"] = {
  coach_id: COACH_ID,
  changes: [
    { acceptance_status: "accepting", reason: null, changed_by: HQ_B, changed_at: "2026-10-30T01:00:00Z" },
    { acceptance_status: "suspended", reason: "連絡がつかない", changed_by: HQ_A, changed_at: "2026-10-28T09:30:00Z" },
  ],
};

function apiOf(result: Result<Schemas["CoachAcceptanceChangeList"]>) {
  return {
    listCoachAcceptanceChanges: vi.fn<HeadquartersApi["listCoachAcceptanceChanges"]>(async () => result),
  };
}

async function choosePeriod(user: ReturnType<typeof userEvent.setup>, from: string, to: string) {
  await user.clear(screen.getByLabelText("開始日"));
  await user.type(screen.getByLabelText("開始日"), from);
  await user.clear(screen.getByLabelText("終了日"));
  await user.type(screen.getByLabelText("終了日"), to);
  await user.click(screen.getByRole("button", { name: "この期間で見る" }));
}

describe("本部のコーチの受付の記録", () => {
  test("読み込み中を出し、読めたら返った順（新しい順）に止めた・再開した・日時・操作した本部のアカウント・理由を出す", async () => {
    const api = apiOf({ ok: true, data: CHANGES });
    render(<CoachAcceptanceHistory api={api} coachId={COACH_ID} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    const table = await screen.findByRole("table", { name: "受付の記録" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();
    expect(api.listCoachAcceptanceChanges).toHaveBeenCalledTimes(1);
    expect(api.listCoachAcceptanceChanges).toHaveBeenCalledWith(COACH_ID, {});

    expect(within(table).getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual([
      "操作",
      "日時",
      "操作した本部のアカウント",
      "理由",
    ]);
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent))).toEqual([
      ["再開した", formatDateTime("2026-10-30T01:00:00Z"), HQ_B, "—"],
      ["止めた", formatDateTime("2026-10-28T09:30:00Z"), HQ_A, "連絡がつかない"],
    ]);
  });

  test("理由を書かずに止めた記録（空文字列）と、理由を残し始める前の記録（null）の理由は「—」", async () => {
    const data: Schemas["CoachAcceptanceChangeList"] = {
      coach_id: COACH_ID,
      changes: [
        { acceptance_status: "suspended", reason: "", changed_by: HQ_A, changed_at: "2026-10-30T01:00:00Z" },
        { acceptance_status: "suspended", reason: null, changed_by: HQ_B, changed_at: "2026-10-01T01:00:00Z" },
      ],
    };
    render(<CoachAcceptanceHistory api={apiOf({ ok: true, data })} coachId={COACH_ID} />);

    const table = await screen.findByRole("table", { name: "受付の記録" });
    const reasons = within(table)
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("cell")[3]!.textContent);
    expect(reasons).toEqual(["—", "—"]);
  });

  test("同じ時刻の記録も返った順のまま出す", async () => {
    const at = "2026-10-30T01:00:00Z";
    const data: Schemas["CoachAcceptanceChangeList"] = {
      coach_id: COACH_ID,
      changes: [
        { acceptance_status: "suspended", reason: "", changed_by: HQ_A, changed_at: at },
        { acceptance_status: "accepting", reason: null, changed_by: HQ_B, changed_at: at },
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
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual(["この期間で見る"]);
  });

  test("期間を付けずに開き、「この期間で見る」で from・to を付けて読み直し、「期間を外す」ですべてに戻す", async () => {
    const user = userEvent.setup();
    const api = apiOf({ ok: true, data: CHANGES });
    render(<CoachAcceptanceHistory api={api} coachId={COACH_ID} />);

    await screen.findByRole("table", { name: "受付の記録" });
    expect(screen.getByLabelText("開始日")).toHaveValue("");
    expect(screen.queryByRole("button", { name: "期間を外す" })).not.toBeInTheDocument();
    await choosePeriod(user, "2026-10-25", "2026-10-31");

    await screen.findByRole("table", { name: "受付の記録" });
    expect(api.listCoachAcceptanceChanges).toHaveBeenCalledTimes(2);
    expect(api.listCoachAcceptanceChanges).toHaveBeenLastCalledWith(COACH_ID, { from: "2026-10-25", to: "2026-10-31" });

    // 確定したあとに欄だけを書き換えても、取り直すときは確定した期間で読む（欄の値は押すまで効かない）。
    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-10-27");
    expect(api.listCoachAcceptanceChanges).toHaveBeenCalledTimes(2);

    await user.click(screen.getByRole("button", { name: "期間を外す" }));
    await screen.findByRole("table", { name: "受付の記録" });
    expect(api.listCoachAcceptanceChanges).toHaveBeenCalledTimes(3);
    expect(api.listCoachAcceptanceChanges).toHaveBeenLastCalledWith(COACH_ID, {});
    expect(screen.getByLabelText("開始日")).toHaveValue("");
    expect(screen.getByLabelText("終了日")).toHaveValue("");
    expect(screen.queryByRole("button", { name: "期間を外す" })).not.toBeInTheDocument();
  });

  test("期間が正しくなければ送らずに直し方を出す（開始日と終了日・終了日は開始日以後・31 日まで）", async () => {
    const user = userEvent.setup();
    const api = apiOf({ ok: true, data: CHANGES });
    render(<CoachAcceptanceHistory api={api} coachId={COACH_ID} />);
    await screen.findByRole("table", { name: "受付の記録" });

    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("開始日と終了日を選んでください。");

    await choosePeriod(user, "2026-10-10", "2026-10-09");
    expect(screen.getByRole("alert")).toHaveTextContent("終了日は開始日と同じか、それより後の日付にしてください。");

    await user.clear(screen.getByLabelText("終了日"));
    await user.type(screen.getByLabelText("終了日"), "2026-11-10");
    await user.click(screen.getByRole("button", { name: "この期間で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("期間は 31 日までにしてください。");

    expect(api.listCoachAcceptanceChanges).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("table", { name: "受付の記録" })).toBeInTheDocument();
  });

  test("期間を付けて 422 なら、サーバーの文言ではなく期間の直し方を出す", async () => {
    const user = userEvent.setup();
    const api = apiOf({ ok: true, data: CHANGES });
    render(<CoachAcceptanceHistory api={api} coachId={COACH_ID} />);
    await screen.findByRole("table", { name: "受付の記録" });

    api.listCoachAcceptanceChanges.mockResolvedValueOnce({
      ok: false,
      code: "validation_failed",
      message: "入力が正しくありません。",
    });
    await choosePeriod(user, "2026-10-01", "2026-10-31");

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "期間の指定が正しくありません。期間は 1〜31 日で、終了日は開始日と同じかそれより後の日付にしてください。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    // 期間の欄は残り、直して押し直せる。
    expect(screen.getByRole("button", { name: "期間を外す" })).toBeInTheDocument();
  });

  test("期間で絞って 0 件なら、その期間を含めた空の文言を出す", async () => {
    const user = userEvent.setup();
    const api = apiOf({ ok: true, data: CHANGES });
    render(<CoachAcceptanceHistory api={api} coachId={COACH_ID} />);
    await screen.findByRole("table", { name: "受付の記録" });

    api.listCoachAcceptanceChanges.mockResolvedValueOnce({ ok: true, data: { coach_id: COACH_ID, changes: [] } });
    await choosePeriod(user, "2026-10-01", "2026-10-07");

    expect(
      await screen.findByText(
        `${formatDay("2026-10-01")} 〜 ${formatDay("2026-10-07")}に受付を止めた・再開した記録はありません`,
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("受付を止めたことはありません")).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("コーチの様子の画面に「受付の記録」の欄があり、コーチの ID がなければ欄を出さない", () => {
    const { unmount } = render(<CoachOverviewHome token="mock.headquarters.abc" coachId={COACH_ID} />);
    expect(screen.getByRole("region", { name: "受付の記録" })).toBeInTheDocument();
    unmount();

    render(<CoachOverviewHome token="mock.headquarters.abc" coachId={undefined} />);
    expect(screen.queryByRole("region", { name: "受付の記録" })).not.toBeInTheDocument();
  });
});
