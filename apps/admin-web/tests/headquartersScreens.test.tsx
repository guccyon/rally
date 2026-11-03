import { render, screen, waitFor, within } from "@testing-library/react";
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

// 日付で期間を指定したときの応答（実際に使った from / to が載る）
const PERIOD_REPORT: Schemas["SlotSupplyReport"] = {
  window_start: "2026-11-01T15:00:00Z",
  window_end: "2026-11-08T15:00:00Z",
  from: "2026-11-02",
  to: "2026-11-08",
  sports: REPORT.sports,
};

function reportOf(report: Schemas["SlotSupplyReport"]) {
  return vi.fn<HeadquartersApi["getSlotSupply"]>(async () => ({ ok: true, data: report }));
}

const SUZUKI = "22222222-2222-4222-8222-000000000001";
const TAKAHASHI = "22222222-2222-4222-8222-000000000002";

const COACHES: Schemas["Coach"][] = [
  { coach_id: SUZUKI, display_name: "鈴木 コーチ", acceptance_status: "accepting", recent_lesson_count: 3 },
  { coach_id: TAKAHASHI, display_name: "高橋 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
];

function coachesOf(coaches: Schemas["Coach"][] = COACHES) {
  return vi.fn<HeadquartersApi["listCoaches"]>(async () => ({ ok: true, data: { coaches } }));
}

// 高橋 コーチで絞ったときの応答（枠を出していないので、どのスポーツも枠が 0）
const EMPTY_COACH_REPORT: Schemas["SlotSupplyReport"] = {
  ...PERIOD_REPORT,
  sports: PERIOD_REPORT.sports.map((supply) => ({
    ...supply,
    status: "sufficient",
    published_slots: 0,
    booking_students: 0,
  })),
};

async function changePeriod(user: ReturnType<typeof userEvent.setup>, from: string, to: string) {
  await user.clear(screen.getByLabelText("開始日"));
  await user.type(screen.getByLabelText("開始日"), from);
  await user.clear(screen.getByLabelText("終了日"));
  await user.type(screen.getByLabelText("終了日"), to);
}

describe("スポーツ別の予約枠の需給", () => {
  test("スポーツごとに判定と判定に使った数を一覧にし、集計の期間を出す", async () => {
    const getSlotSupply = vi.fn<HeadquartersApi["getSlotSupply"]>(async () => ({
      ok: true,
      data: REPORT,
    }));
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);

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
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("再集計で API を呼び直す", async () => {
    const getSlotSupply = vi.fn<HeadquartersApi["getSlotSupply"]>(async () => ({
      ok: true,
      data: REPORT,
    }));
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: "再集計" }));

    await screen.findByRole("table");
    expect(getSlotSupply).toHaveBeenCalledTimes(2);
  });

  test("開いたときは期間を付けずに読み（API の既定の 30 日）、数えた期間の日付を期間の欄に入れる", async () => {
    const getSlotSupply = reportOf(REPORT);
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);

    await screen.findByRole("table");
    expect(getSlotSupply).toHaveBeenCalledWith({});
    // window_end は含まないので、終わりの日付はその直前の日付（日本時間）
    expect(screen.getByLabelText("開始日")).toHaveValue("2026-09-27");
    expect(screen.getByLabelText("終了日")).toHaveValue("2026-10-27");
  });

  test("期間を選んで「この条件で見る」で API の from / to に渡して読み直し、使った期間を表の上に出す", async () => {
    const getSlotSupply = vi
      .fn<HeadquartersApi["getSlotSupply"]>()
      .mockResolvedValueOnce({ ok: true, data: REPORT })
      .mockResolvedValueOnce({ ok: true, data: PERIOD_REPORT });
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-11-02", "2026-11-08");
    expect(getSlotSupply).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-11-02", to: "2026-11-08" });
    expect(await screen.findByText(/2026年11月2日（月） 〜 2026年11月8日（日）/)).toHaveTextContent("集計の期間");
    expect(screen.getByRole("table")).toBeInTheDocument();
  });

  test("再集計は最後に「この条件で見る」で読んだ期間で読み直す", async () => {
    const getSlotSupply = reportOf(PERIOD_REPORT);
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-11-02", "2026-11-08");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await screen.findByRole("table");
    // 欄を変えただけでは再集計の期間は変わらない
    await changePeriod(user, "2026-12-01", "2026-12-07");
    await user.click(screen.getByRole("button", { name: "再集計" }));

    await screen.findByRole("table");
    expect(getSlotSupply).toHaveBeenCalledTimes(3);
    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-11-02", to: "2026-11-08" });
  });

  test("終了日が開始日より前・31 日を超える期間は送らずに理由を出す", async () => {
    const getSlotSupply = reportOf(REPORT);
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-11-08", "2026-11-07");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("終了日は開始日と同じか、それより後の日付にしてください。");

    await changePeriod(user, "2026-11-01", "2026-12-02");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    expect(screen.getByRole("alert")).toHaveTextContent("期間は 31 日までにしてください。");
    expect(getSlotSupply).toHaveBeenCalledTimes(1);
  });

  test("API の 422 は、期間のどこを直すかを添えた理由を出し、一覧は出さない", async () => {
    const getSlotSupply = vi.fn<HeadquartersApi["getSlotSupply"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力の内容が正しくありません。",
    }));
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "期間・コーチ・スポーツの指定が正しくありません。期間は 1〜31 日で、終了日は開始日と同じかそれより後の日付にしてください。コーチとスポーツは一覧から選び直してください。",
    );
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("どのスポーツにも枠も予約しようとした生徒もなければ、期間とともに 0 件の文言を出し、表は出さない", async () => {
    const empty: Schemas["SlotSupplyReport"] = {
      ...PERIOD_REPORT,
      sports: PERIOD_REPORT.sports.map((supply) => ({
        ...supply,
        status: "sufficient",
        published_slots: 0,
        booking_students: 0,
      })),
    };
    render(<SlotSupplyReport api={{ getSlotSupply: reportOf(empty), listCoaches: coachesOf() }} />);

    expect(await screen.findByText("この期間に開始する予約枠はなく、予約しようとした生徒もいません")).toBeInTheDocument();
    expect(screen.getByText(/集計の期間/)).toHaveTextContent("2026年11月2日（月） 〜 2026年11月8日（日）");
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  test("後から変えた期間の結果だけを出す（前の期間の遅れた結果で上書きしない）", async () => {
    let resolveSlow: (value: Awaited<ReturnType<HeadquartersApi["getSlotSupply"]>>) => void = () => {};
    const getSlotSupply = vi
      .fn<HeadquartersApi["getSlotSupply"]>()
      .mockResolvedValueOnce({ ok: true, data: REPORT })
      .mockImplementationOnce(() => new Promise((resolve) => (resolveSlow = resolve)))
      .mockResolvedValueOnce({ ok: true, data: PERIOD_REPORT });
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-10-01", "2026-10-07");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await changePeriod(user, "2026-11-02", "2026-11-08");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await screen.findByText(/2026年11月2日（月）/);
    resolveSlow({ ok: true, data: { ...PERIOD_REPORT, from: "2026-10-01", to: "2026-10-07" } });

    await waitFor(() => expect(screen.getByText(/集計の期間/)).toHaveTextContent("2026年11月2日（月）"));
  });

  test("コーチの選択肢はコーチの一覧の順で、コーチを選んで「この条件で見る」で API の coach_id に渡して読み直し、どのコーチの需給かを見出しに出す", async () => {
    const getSlotSupply = reportOf(REPORT);
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    const select = screen.getByRole("combobox", { name: "コーチ" });
    await waitFor(() => expect(select).toBeEnabled());
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "鈴木 コーチ",
      "高橋 コーチ",
    ]);
    // 開いたとき（すべてのコーチ）は見出しを出さない
    expect(screen.queryByRole("heading", { level: 3 })).not.toBeInTheDocument();

    await user.selectOptions(select, "鈴木 コーチ");
    expect(getSlotSupply).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(await screen.findByRole("heading", { name: "鈴木 コーチ の予約枠の需給" })).toBeInTheDocument();
    // 開いたときに入れた期間（API の既定の 30 日の日付）と組み合わせて送る
    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-09-27", to: "2026-10-27", coach_id: SUZUKI });

    // 「すべて」に戻すと coach_id を付けずに読み直し、見出しを消す
    await user.selectOptions(select, "すべて");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await waitFor(() => expect(getSlotSupply).toHaveBeenCalledTimes(3));
    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-09-27", to: "2026-10-27" });
    await screen.findByRole("table");
    expect(screen.queryByRole("heading", { level: 3 })).not.toBeInTheDocument();
  });

  test("期間とコーチを組み合わせて API の from / to / coach_id に渡し、再集計も同じ条件で読み直す。0 件ならそのコーチの枠のことだと言う", async () => {
    const getSlotSupply = vi
      .fn<HeadquartersApi["getSlotSupply"]>()
      .mockResolvedValueOnce({ ok: true, data: REPORT })
      .mockResolvedValue({ ok: true, data: EMPTY_COACH_REPORT });
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toBeEnabled());

    await changePeriod(user, "2026-11-02", "2026-11-08");
    await user.selectOptions(screen.getByRole("combobox", { name: "コーチ" }), "高橋 コーチ");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-11-02", to: "2026-11-08", coach_id: TAKAHASHI });
    expect(await screen.findByRole("heading", { name: "高橋 コーチ の予約枠の需給" })).toBeInTheDocument();
    expect(
      screen.getByText("この期間に開始するこのコーチの予約枠はなく、予約しようとした生徒もいません"),
    ).toBeInTheDocument();
    expect(screen.getByText(/集計の期間/)).toHaveTextContent("2026年11月2日（月） 〜 2026年11月8日（日）");

    // 欄を変えただけでは再集計の条件は変わらない
    await user.selectOptions(screen.getByRole("combobox", { name: "コーチ" }), "すべて");
    await user.click(screen.getByRole("button", { name: "再集計" }));
    await screen.findByRole("heading", { name: "高橋 コーチ の予約枠の需給" });
    expect(getSlotSupply).toHaveBeenCalledTimes(3);
    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-11-02", to: "2026-11-08", coach_id: TAKAHASHI });
  });

  test("スポーツの選択肢は契約の順で、スポーツを選んで「この条件で見る」で API の sport に渡して読み直し、どの競技の需給かを見出しに出す", async () => {
    const getSlotSupply = reportOf(REPORT);
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    const select = screen.getByRole("combobox", { name: "スポーツ" });
    expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "すべて",
      "テニス",
      "ゴルフ",
      "野球",
      "サッカー",
      "バスケットボール",
      "水泳",
    ]);

    await user.selectOptions(select, "ゴルフ");
    expect(getSlotSupply).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(await screen.findByRole("heading", { name: "ゴルフを提供するコーチの予約枠の需給" })).toBeInTheDocument();
    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-09-27", to: "2026-10-27", sport: "golf" });
    // 表は今までどおり 6 種類すべてを出す（契約）
    expect(screen.getAllByRole("row")).toHaveLength(7);

    // 「すべて」に戻すと sport を付けずに読み直し、見出しを消す
    await user.selectOptions(select, "すべて");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));
    await waitFor(() => expect(getSlotSupply).toHaveBeenCalledTimes(3));
    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-09-27", to: "2026-10-27" });
    await screen.findByRole("table");
    expect(screen.queryByRole("heading", { level: 3 })).not.toBeInTheDocument();
  });

  test("スポーツだけで絞って 0 件なら、その競技のコーチの枠のことだと言う", async () => {
    const getSlotSupply = vi
      .fn<HeadquartersApi["getSlotSupply"]>()
      .mockResolvedValueOnce({ ok: true, data: REPORT })
      .mockResolvedValue({ ok: true, data: EMPTY_COACH_REPORT });
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");

    await changePeriod(user, "2026-11-02", "2026-11-08");
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "水泳");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(getSlotSupply).toHaveBeenLastCalledWith({ from: "2026-11-02", to: "2026-11-08", sport: "swimming" });
    expect(await screen.findByRole("heading", { name: "水泳を提供するコーチの予約枠の需給" })).toBeInTheDocument();
    expect(
      screen.getByText("この期間に開始するこのスポーツのコーチの予約枠はなく、予約しようとした生徒もいません"),
    ).toBeInTheDocument();
  });

  test("期間・コーチ・スポーツを組み合わせて API の from / to / coach_id / sport に渡し、再集計も同じ条件で読み直す", async () => {
    const getSlotSupply = vi
      .fn<HeadquartersApi["getSlotSupply"]>()
      .mockResolvedValueOnce({ ok: true, data: REPORT })
      .mockResolvedValue({ ok: true, data: EMPTY_COACH_REPORT });
    const user = userEvent.setup();
    render(<SlotSupplyReport api={{ getSlotSupply, listCoaches: coachesOf() }} />);
    await screen.findByRole("table");
    await waitFor(() => expect(screen.getByRole("combobox", { name: "コーチ" })).toBeEnabled());

    await changePeriod(user, "2026-11-02", "2026-11-08");
    await user.selectOptions(screen.getByRole("combobox", { name: "コーチ" }), "鈴木 コーチ");
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "テニス");
    await user.click(screen.getByRole("button", { name: "この条件で見る" }));

    expect(getSlotSupply).toHaveBeenLastCalledWith({
      from: "2026-11-02",
      to: "2026-11-08",
      coach_id: SUZUKI,
      sport: "tennis",
    });
    expect(
      await screen.findByRole("heading", { name: "鈴木 コーチ の予約枠の需給（テニスを提供するコーチに限る）" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("この期間に開始する条件に合う予約枠はなく、予約しようとした生徒もいません"),
    ).toBeInTheDocument();

    // 欄を変えただけでは再集計の条件は変わらない
    await user.selectOptions(screen.getByRole("combobox", { name: "スポーツ" }), "すべて");
    await user.click(screen.getByRole("button", { name: "再集計" }));
    await screen.findByRole("heading", { name: "鈴木 コーチ の予約枠の需給（テニスを提供するコーチに限る）" });
    expect(getSlotSupply).toHaveBeenCalledTimes(3);
    expect(getSlotSupply).toHaveBeenLastCalledWith({
      from: "2026-11-02",
      to: "2026-11-08",
      coach_id: SUZUKI,
      sport: "tennis",
    });
  });

  test("コーチの一覧が読めなくても期間だけで需給を出し、コーチの絞り込みだけを使えなくする", async () => {
    const listCoaches = vi.fn<HeadquartersApi["listCoaches"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<SlotSupplyReport api={{ getSlotSupply: reportOf(REPORT), listCoaches }} />);

    expect(await screen.findByRole("table")).toBeInTheDocument();
    expect(await screen.findByRole("alert")).toHaveTextContent("コーチの一覧を読めませんでした。サーバーに接続できません。");
    expect(screen.getByRole("combobox", { name: "コーチ" })).toBeDisabled();
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
