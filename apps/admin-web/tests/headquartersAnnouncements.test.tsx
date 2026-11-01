import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";

import type { Schemas } from "@/api/client";
import type { HeadquartersApi } from "@/api/headquarters";
import { RoleGate } from "@/components/RoleGate";
import { Announcements, type AnnouncementsApi } from "@/components/headquarters/Announcements";
import { AnnouncementsHome } from "@/components/headquarters/AnnouncementsHome";
import { HeadquartersHome } from "@/components/headquarters/HeadquartersHome";
import { saveSession } from "@/session/session";

const replace = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), replace }) }));

beforeEach(() => {
  replace.mockReset();
});

const SENDER_ID = "99999999-9999-4999-8999-999999999999";

const SENT: Schemas["Announcement"][] = [
  {
    announcement_id: "dddddddd-0000-4000-8000-000000000002",
    sender_id: SENDER_ID,
    title: "水泳のキャンペーン",
    body: "今月中に体験予約すると\n2 回目が無料です。",
    sport: "swimming",
    recipient_count: 8,
    sent_at: "2026-10-25T01:30:00Z",
    withdrawal: null,
  },
  {
    announcement_id: "dddddddd-0000-4000-8000-000000000001",
    sender_id: SENDER_ID,
    title: "メンテナンスのお知らせ",
    body: "10月30日 2:00〜4:00 は使えません。",
    sport: null,
    recipient_count: 120,
    sent_at: "2026-10-20T00:00:00Z",
    withdrawal: null,
  },
];

function listOf(announcements: Schemas["Announcement"][]) {
  return vi.fn<HeadquartersApi["listAnnouncements"]>(async () => ({
    ok: true,
    data: { announcements },
  }));
}

function sendOk() {
  return vi.fn<HeadquartersApi["sendAnnouncement"]>(async (request) => ({
    ok: true,
    data: {
      announcement_id: "dddddddd-0000-4000-8000-000000000003",
      sender_id: SENDER_ID,
      title: request.title,
      body: request.body,
      sport: request.sport ?? null,
      recipient_count: 30,
      sent_at: "2026-10-26T03:00:00Z",
      withdrawal: null,
    },
  }));
}

function draftListOf(drafts: Schemas["AnnouncementDraft"][]) {
  return vi.fn<HeadquartersApi["listAnnouncementDrafts"]>(async () => ({
    ok: true,
    data: { announcement_drafts: drafts },
  }));
}

/** 下書きの操作を足した API。渡さなかった操作は、一覧は空・ほかは呼ばれたら失敗にする。 */
function apiOf(overrides: Partial<AnnouncementsApi>): AnnouncementsApi {
  const unexpected = async () => ({ ok: false as const, message: "想定外の呼び出し" });
  return {
    listAnnouncements: listOf([]),
    sendAnnouncement: sendOk(),
    listAnnouncementDrafts: draftListOf([]),
    createAnnouncementDraft: vi.fn(unexpected),
    updateAnnouncementDraft: vi.fn(unexpected),
    deleteAnnouncementDraft: vi.fn(unexpected),
    sendAnnouncementDraft: vi.fn(unexpected),
    ...overrides,
  };
}

function items() {
  return within(screen.getByRole("list", { name: "送ったお知らせの一覧" })).getAllByRole("listitem");
}

async function fillAndReview(
  user: ReturnType<typeof userEvent.setup>,
  { target, title, body }: { target?: string; title: string; body: string },
) {
  if (target) await user.selectOptions(screen.getByLabelText("対象"), target);
  if (title) await user.type(screen.getByLabelText("題"), title);
  if (body) await user.type(screen.getByLabelText("本文"), body);
  await user.click(screen.getByRole("button", { name: "内容を確かめる" }));
}

describe("送ったお知らせの一覧", () => {
  test("読み込み中を出し、読めたら API の順に題・対象・受け取った人数・送った日時・本文を出す", async () => {
    const listAnnouncements = listOf(SENT);
    render(<Announcements api={apiOf({ listAnnouncements, sendAnnouncement: sendOk() })} />);

    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
    await screen.findByRole("list", { name: "送ったお知らせの一覧" });
    expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument();

    const [first, second] = items();
    expect(first).toHaveTextContent("水泳のキャンペーン");
    expect(first).toHaveTextContent("水泳の生徒・8 人に送信・2026/10/25 10:30:00");
    // 本文の改行はそのまま
    expect(within(first!).getByText(/今月中に体験予約すると/).textContent).toBe(
      "今月中に体験予約すると\n2 回目が無料です。",
    );
    expect(second).toHaveTextContent("メンテナンスのお知らせ");
    expect(second).toHaveTextContent("生徒全員・120 人に送信・2026/10/20 9:00:00");
    expect(listAnnouncements).toHaveBeenCalledTimes(1);
  });

  test("0 件なら「まだお知らせを送っていません」と出し、一覧は出さない", async () => {
    render(<Announcements api={apiOf({ listAnnouncements: listOf([]), sendAnnouncement: sendOk() })} />);

    expect(await screen.findByText("まだお知らせを送っていません")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "送ったお知らせの一覧" })).not.toBeInTheDocument();
  });

  test("API のエラー（本部以外のトークンの 403 など）の文言を出し、一覧は出さない", async () => {
    const listAnnouncements = vi.fn<HeadquartersApi["listAnnouncements"]>(async () => ({
      ok: false,
      message: "権限がないためアクセスできません。",
    }));
    render(<Announcements api={apiOf({ listAnnouncements, sendAnnouncement: sendOk() })} />);

    const list = screen.getByRole("region", { name: "送ったお知らせ" });
    expect(await within(list).findByRole("alert")).toHaveTextContent("権限がないためアクセスできません。");
    expect(screen.queryByRole("list", { name: "送ったお知らせの一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("まだお知らせを送っていません")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("読み込み中…")).not.toBeInTheDocument());
  });
});

describe("お知らせを送る", () => {
  test("対象は「生徒全員」（既定）と 6 種類のスポーツから選べる", async () => {
    render(<Announcements api={apiOf({ listAnnouncements: listOf([]), sendAnnouncement: sendOk() })} />);
    await screen.findByText("まだお知らせを送っていません");

    expect(screen.getByLabelText("対象")).toHaveValue("");
    const options = within(screen.getByLabelText("対象")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "生徒全員（ゴルフの生徒も含む）",
      "テニスの生徒",
      "ゴルフの生徒",
      "野球の生徒",
      "サッカーの生徒",
      "バスケットボールの生徒",
      "水泳の生徒",
    ]);
  });

  test("送る前にページの中で対象・題・本文を確かめ、確定で送り、一覧の先頭に足して入力を空にする", async () => {
    const user = userEvent.setup();
    const sendAnnouncement = sendOk();
    const listAnnouncements = listOf(SENT);
    render(<Announcements api={apiOf({ listAnnouncements, sendAnnouncement })} />);
    await screen.findByRole("list", { name: "送ったお知らせの一覧" });

    await fillAndReview(user, { target: "テニスの生徒", title: "コートの案内", body: "1 行目{Enter}2 行目" });

    // 確かめている間は送らない
    expect(sendAnnouncement).not.toHaveBeenCalled();
    const confirm = screen.getByRole("group", { name: "送る前の確認" });
    expect(confirm).toHaveTextContent("次の内容でテニスの生徒に送りますか？");
    // 取り下げ（withdrawAnnouncement）はできても、届いたプッシュ通知は取り消せない（契約）
    expect(confirm).toHaveTextContent("送るとすぐに生徒に届き、プッシュ通知は取り消せません。");
    expect(within(confirm).getByText("コートの案内")).toBeInTheDocument();
    expect(within(confirm).getByText(/1 行目/).textContent).toBe("1 行目\n2 行目");

    await user.click(screen.getByRole("button", { name: "送るのを確定" }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      "お知らせ「コートの案内」を 30 人の生徒に送りました。",
    );
    expect(sendAnnouncement).toHaveBeenCalledWith({
      title: "コートの案内",
      body: "1 行目\n2 行目",
      sport: "tennis",
    });
    expect(items().map((item) => item.querySelector("p")?.textContent)).toEqual([
      "コートの案内",
      "水泳のキャンペーン",
      "メンテナンスのお知らせ",
    ]);
    expect(listAnnouncements).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText("題")).toHaveValue("");
    expect(screen.getByLabelText("本文")).toHaveValue("");
    expect(screen.getByLabelText("対象")).toHaveValue("");
  });

  test("対象が生徒全員なら sport を null で送る", async () => {
    const user = userEvent.setup();
    const sendAnnouncement = sendOk();
    render(<Announcements api={apiOf({ listAnnouncements: listOf([]), sendAnnouncement })} />);
    await screen.findByText("まだお知らせを送っていません");

    await fillAndReview(user, { title: "メンテナンス", body: "止まります" });
    expect(screen.getByRole("group", { name: "送る前の確認" })).toHaveTextContent(
      "次の内容で生徒全員に送りますか？",
    );
    await user.click(screen.getByRole("button", { name: "送るのを確定" }));

    await screen.findByRole("status");
    expect(sendAnnouncement).toHaveBeenCalledWith({ title: "メンテナンス", body: "止まります", sport: null });
    // 0 件の表示から、送ったお知らせの一覧に変わる
    expect(items()).toHaveLength(1);
    expect(screen.queryByText("まだお知らせを送っていません")).not.toBeInTheDocument();
  });

  test("「直す」で入力に戻り、書いた内容は残る", async () => {
    const user = userEvent.setup();
    const sendAnnouncement = sendOk();
    render(<Announcements api={apiOf({ listAnnouncements: listOf([]), sendAnnouncement })} />);
    await screen.findByText("まだお知らせを送っていません");

    await fillAndReview(user, { target: "野球の生徒", title: "題です", body: "本文です" });
    await user.click(screen.getByRole("button", { name: "直す" }));

    expect(screen.queryByRole("group", { name: "送る前の確認" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("対象")).toHaveValue("baseball");
    expect(screen.getByLabelText("題")).toHaveValue("題です");
    expect(screen.getByLabelText("本文")).toHaveValue("本文です");
    expect(sendAnnouncement).not.toHaveBeenCalled();
  });

  test("空・空白だけの題と本文は確かめに進まず、項目の下に理由を出す", async () => {
    const user = userEvent.setup();
    const sendAnnouncement = sendOk();
    render(<Announcements api={apiOf({ listAnnouncements: listOf([]), sendAnnouncement })} />);
    await screen.findByText("まだお知らせを送っていません");

    await fillAndReview(user, { title: "   ", body: "" });

    expect(screen.queryByRole("group", { name: "送る前の確認" })).not.toBeInTheDocument();
    expect(screen.getByText("題を入力してください。")).toBeInTheDocument();
    expect(screen.getByText("本文を入力してください。")).toBeInTheDocument();
    expect(screen.getByLabelText("題")).toHaveAttribute("aria-invalid", "true");
    expect(sendAnnouncement).not.toHaveBeenCalled();
  });

  test("題は 50 文字、本文は 2000 文字まで（コードポイントで数える）。超えたら残りの文字数と理由を出し、確かめに進まない", async () => {
    const user = userEvent.setup();
    render(<Announcements api={apiOf({ listAnnouncements: listOf([]), sendAnnouncement: sendOk() })} />);
    await screen.findByText("まだお知らせを送っていません");

    // 絵文字は UTF-16 では 2 単位だが 1 文字
    await user.click(screen.getByLabelText("題"));
    await user.paste("🎾".repeat(50));
    expect(screen.getByText("あと 0 文字")).toBeInTheDocument();
    await user.type(screen.getByLabelText("題"), "あ");
    expect(screen.getByText("50 文字を 1 文字超えています")).toBeInTheDocument();

    await user.click(screen.getByLabelText("本文"));
    await user.paste("あ".repeat(2001));
    await user.click(screen.getByRole("button", { name: "内容を確かめる" }));

    expect(screen.queryByRole("group", { name: "送る前の確認" })).not.toBeInTheDocument();
    expect(screen.getByText("題は 50 文字までです。1 文字減らしてください。")).toBeInTheDocument();
    expect(screen.getByText("本文は 2000 文字までです。1 文字減らしてください。")).toBeInTheDocument();
  });

  test("送っている間は確定を押せず「送っています…」と出す", async () => {
    const user = userEvent.setup();
    let resolve: (value: Awaited<ReturnType<HeadquartersApi["sendAnnouncement"]>>) => void = () => {};
    const sendAnnouncement = vi.fn<HeadquartersApi["sendAnnouncement"]>(
      () => new Promise((r) => (resolve = r)),
    );
    render(<Announcements api={apiOf({ listAnnouncements: listOf([]), sendAnnouncement })} />);
    await screen.findByText("まだお知らせを送っていません");

    await fillAndReview(user, { title: "題", body: "本文" });
    await user.click(screen.getByRole("button", { name: "送るのを確定" }));

    expect(screen.getByRole("button", { name: "送っています…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "直す" })).toBeDisabled();
    resolve({ ok: false, message: "サーバーに接続できません。" });
    expect(await screen.findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
  });

  test("対象の生徒がいない（404）ときは、その旨を出して入力を残し、一覧は変えない", async () => {
    const user = userEvent.setup();
    const sendAnnouncement = vi.fn<HeadquartersApi["sendAnnouncement"]>(async () => ({
      ok: false,
      code: "not_found",
      message: "対象が見つかりません（ID を確かめてください）。",
    }));
    render(<Announcements api={apiOf({ listAnnouncements: listOf(SENT), sendAnnouncement })} />);
    await screen.findByRole("list", { name: "送ったお知らせの一覧" });

    await fillAndReview(user, { target: "水泳の生徒", title: "題", body: "本文" });
    await user.click(screen.getByRole("button", { name: "送るのを確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "対象の生徒がいないため、お知らせは送られませんでした。対象を確かめてください。",
    );
    expect(screen.getByLabelText("対象")).toHaveValue("swimming");
    expect(screen.getByLabelText("題")).toHaveValue("題");
    expect(items()).toHaveLength(2);
  });

  test("ほかのエラー（422 など）はサーバーの ErrorCode の文言を出して入力を残す", async () => {
    const user = userEvent.setup();
    const sendAnnouncement = vi.fn<HeadquartersApi["sendAnnouncement"]>(async () => ({
      ok: false,
      code: "validation_failed",
      message: "入力の内容が正しくありません。",
    }));
    render(<Announcements api={apiOf({ listAnnouncements: listOf([]), sendAnnouncement })} />);
    await screen.findByText("まだお知らせを送っていません");

    await fillAndReview(user, { title: "題", body: "本文" });
    await user.click(screen.getByRole("button", { name: "送るのを確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("入力の内容が正しくありません。");
    expect(screen.getByLabelText("本文")).toHaveValue("本文");
  });
});

const DRAFTS: Schemas["AnnouncementDraft"][] = [
  {
    announcement_draft_id: "eeeeeeee-0000-4000-8000-000000000002",
    created_by: SENDER_ID,
    title: "年末年始の営業",
    body: "12月29日から1月3日はお休みです。",
    sport: null,
    created_at: "2026-10-28T00:00:00Z",
    updated_at: "2026-10-30T03:15:00Z",
  },
  {
    announcement_draft_id: "eeeeeeee-0000-4000-8000-000000000001",
    created_by: SENDER_ID,
    title: "",
    body: "テニスの新しいコーチの紹介（書きかけ）",
    sport: "tennis",
    created_at: "2026-10-27T00:00:00Z",
    updated_at: "2026-10-27T00:00:00Z",
  },
];

function draftItems() {
  return within(screen.getByRole("list", { name: "下書きの一覧" })).getAllByRole("listitem");
}

function savedDraft(
  id: string,
  request: Schemas["AnnouncementDraftRequest"],
  updatedAt = "2026-11-01T02:00:00Z",
): Schemas["AnnouncementDraft"] {
  return {
    announcement_draft_id: id,
    created_by: SENDER_ID,
    title: request.title,
    body: request.body,
    sport: request.sport ?? null,
    created_at: "2026-10-27T00:00:00Z",
    updated_at: updatedAt,
  };
}

function updateOk() {
  return vi.fn<HeadquartersApi["updateAnnouncementDraft"]>(async (id, request) => ({
    ok: true,
    data: savedDraft(id, request),
  }));
}

function sendDraftOk() {
  return vi.fn<HeadquartersApi["sendAnnouncementDraft"]>(async () => ({
    ok: true,
    data: {
      announcement_id: "dddddddd-0000-4000-8000-000000000009",
      sender_id: SENDER_ID,
      title: "年末年始の営業",
      body: "12月29日から1月3日はお休みです。",
      sport: null,
      recipient_count: 120,
      sent_at: "2026-11-01T02:00:00Z",
      withdrawal: null,
    },
  }));
}

describe("下書きの一覧", () => {
  test("下書きを読み込み中を出し、読めたら API の順に題（空なら「（題なし）」）・対象・最後に直した日時を出す", async () => {
    const listAnnouncementDrafts = draftListOf(DRAFTS);
    render(<Announcements api={apiOf({ listAnnouncementDrafts })} />);

    expect(screen.getByText("下書きを読み込み中…")).toBeInTheDocument();
    await screen.findByRole("list", { name: "下書きの一覧" });

    const [first, second] = draftItems();
    expect(first).toHaveTextContent("年末年始の営業");
    expect(first).toHaveTextContent("生徒全員・最後に直した日時 2026/10/30 12:15:00");
    expect(second).toHaveTextContent("（題なし）");
    expect(second).toHaveTextContent("テニスの生徒・最後に直した日時 2026/10/27 9:00:00");
    // 下書きは生徒に届かないことを画面に書く
    expect(screen.getByRole("region", { name: "下書き" })).toHaveTextContent("下書きは生徒に届きません");
    expect(listAnnouncementDrafts).toHaveBeenCalledTimes(1);
  });

  test("0 件なら「保存した下書きはありません」と出し、一覧は出さない", async () => {
    render(<Announcements api={apiOf({})} />);

    expect(await screen.findByText("保存した下書きはありません")).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "下書きの一覧" })).not.toBeInTheDocument();
  });

  test("読み込めなければその文言を出し、一覧は出さない", async () => {
    const listAnnouncementDrafts = vi.fn<HeadquartersApi["listAnnouncementDrafts"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<Announcements api={apiOf({ listAnnouncementDrafts })} />);

    const region = screen.getByRole("region", { name: "下書き" });
    expect(await within(region).findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(screen.queryByRole("list", { name: "下書きの一覧" })).not.toBeInTheDocument();
    expect(screen.queryByText("下書きを読み込み中…")).not.toBeInTheDocument();
  });
});

describe("下書きに保存する", () => {
  test("書きかけ（題が空）でも保存でき、生徒には送らず一覧の先頭に足す。続けて保存するとその下書きを直す", async () => {
    const user = userEvent.setup();
    const sendAnnouncement = sendOk();
    const createAnnouncementDraft = vi.fn<HeadquartersApi["createAnnouncementDraft"]>(async (request) => ({
      ok: true,
      data: savedDraft("eeeeeeee-0000-4000-8000-000000000003", request),
    }));
    const updateAnnouncementDraft = updateOk();
    render(
      <Announcements
        api={apiOf({
          sendAnnouncement,
          createAnnouncementDraft,
          updateAnnouncementDraft,
          listAnnouncementDrafts: draftListOf(DRAFTS),
        })}
      />,
    );
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.selectOptions(screen.getByLabelText("対象"), "水泳の生徒");
    await user.type(screen.getByLabelText("本文"), "書きかけ");
    await user.click(screen.getByRole("button", { name: "下書きに保存" }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      "下書き「（題なし）」を保存しました。生徒にはまだ届いていません。",
    );
    expect(createAnnouncementDraft).toHaveBeenCalledWith({ title: "", body: "書きかけ", sport: "swimming" });
    expect(sendAnnouncement).not.toHaveBeenCalled();
    expect(draftItems()).toHaveLength(3);
    expect(draftItems()[0]).toHaveTextContent("（題なし）（開いています）");
    // 入力は残る
    expect(screen.getByLabelText("本文")).toHaveValue("書きかけ");

    await user.type(screen.getByLabelText("題"), "水泳の案内");
    await user.click(screen.getByRole("button", { name: "下書きに保存" }));

    expect(await screen.findByText("下書き「水泳の案内」を保存しました。生徒にはまだ届いていません。")).toBeInTheDocument();
    expect(createAnnouncementDraft).toHaveBeenCalledTimes(1);
    expect(updateAnnouncementDraft).toHaveBeenCalledWith("eeeeeeee-0000-4000-8000-000000000003", {
      title: "水泳の案内",
      body: "書きかけ",
      sport: "swimming",
    });
    expect(draftItems()).toHaveLength(3);
    expect(draftItems()[0]).toHaveTextContent("水泳の案内");
  });

  test("長すぎる題・本文は保存せず、項目の下に理由を出す", async () => {
    const user = userEvent.setup();
    const createAnnouncementDraft = vi.fn<HeadquartersApi["createAnnouncementDraft"]>();
    render(<Announcements api={apiOf({ createAnnouncementDraft })} />);
    await screen.findByText("保存した下書きはありません");

    await user.click(screen.getByLabelText("題"));
    await user.paste("あ".repeat(51));
    await user.click(screen.getByRole("button", { name: "下書きに保存" }));

    expect(screen.getByText("題は 50 文字までです。1 文字減らしてください。")).toBeInTheDocument();
    // 空の本文は下書きでは断らない
    expect(screen.queryByText("本文を入力してください。")).not.toBeInTheDocument();
    expect(createAnnouncementDraft).not.toHaveBeenCalled();
  });

  test("保存に失敗したらその文言を出して入力を残す", async () => {
    const user = userEvent.setup();
    const createAnnouncementDraft = vi.fn<HeadquartersApi["createAnnouncementDraft"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<Announcements api={apiOf({ createAnnouncementDraft })} />);
    await screen.findByText("保存した下書きはありません");

    await user.type(screen.getByLabelText("題"), "題");
    await user.click(screen.getByRole("button", { name: "下書きに保存" }));

    const form = screen.getByRole("region", { name: "お知らせを送る" });
    expect(await within(form).findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(screen.getByLabelText("題")).toHaveValue("題");
    expect(screen.getByText("保存した下書きはありません")).toBeInTheDocument();
  });
});

describe("下書きを開いて直す・送る・消す", () => {
  test("「開いて直す」で中身をフォームに入れ、保存するとその下書きを直して一覧の先頭に移す", async () => {
    const user = userEvent.setup();
    const updateAnnouncementDraft = updateOk();
    render(<Announcements api={apiOf({ listAnnouncementDrafts: draftListOf(DRAFTS), updateAnnouncementDraft })} />);
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.click(within(draftItems()[1]!).getByRole("button", { name: "開いて直す" }));

    expect(screen.getByRole("region", { name: "お知らせを送る" })).toHaveTextContent(
      "下書き「（題なし）」を開いています。",
    );
    expect(screen.getByLabelText("対象")).toHaveValue("tennis");
    expect(screen.getByLabelText("題")).toHaveValue("");
    expect(screen.getByLabelText("本文")).toHaveValue("テニスの新しいコーチの紹介（書きかけ）");

    await user.type(screen.getByLabelText("題"), "新しいコーチ");
    await user.click(screen.getByRole("button", { name: "下書きに保存" }));

    await screen.findByText("下書き「新しいコーチ」を保存しました。生徒にはまだ届いていません。");
    expect(updateAnnouncementDraft).toHaveBeenCalledWith("eeeeeeee-0000-4000-8000-000000000001", {
      title: "新しいコーチ",
      body: "テニスの新しいコーチの紹介（書きかけ）",
      sport: "tennis",
    });
    expect(draftItems().map((item) => item.querySelector("p")?.textContent)).toEqual([
      "新しいコーチ（開いています）",
      "年末年始の営業",
    ]);
  });

  test("「新しく書く」で下書きを閉じて入力を空にし、保存すると新しい下書きを作る", async () => {
    const user = userEvent.setup();
    const createAnnouncementDraft = vi.fn<HeadquartersApi["createAnnouncementDraft"]>(async (request) => ({
      ok: true,
      data: savedDraft("eeeeeeee-0000-4000-8000-000000000004", request),
    }));
    const updateAnnouncementDraft = updateOk();
    render(
      <Announcements
        api={apiOf({ listAnnouncementDrafts: draftListOf(DRAFTS), createAnnouncementDraft, updateAnnouncementDraft })}
      />,
    );
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.click(within(draftItems()[0]!).getByRole("button", { name: "開いて直す" }));
    await user.click(screen.getByRole("button", { name: "新しく書く" }));

    expect(screen.getByLabelText("題")).toHaveValue("");
    expect(screen.queryByText(/を開いています/)).not.toBeInTheDocument();
    await user.type(screen.getByLabelText("題"), "別の案内");
    await user.click(screen.getByRole("button", { name: "下書きに保存" }));

    await screen.findByRole("status");
    expect(createAnnouncementDraft).toHaveBeenCalledTimes(1);
    expect(updateAnnouncementDraft).not.toHaveBeenCalled();
  });

  test("一覧の「送る」で今と同じ確認に進み、確定で中身を直してから下書きから送る。送れたら下書きは消え、送ったお知らせの先頭に足す", async () => {
    const user = userEvent.setup();
    const sendAnnouncement = sendOk();
    const updateAnnouncementDraft = updateOk();
    const sendAnnouncementDraft = sendDraftOk();
    render(
      <Announcements
        api={apiOf({
          listAnnouncements: listOf(SENT),
          sendAnnouncement,
          listAnnouncementDrafts: draftListOf(DRAFTS),
          updateAnnouncementDraft,
          sendAnnouncementDraft,
        })}
      />,
    );
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.click(within(draftItems()[0]!).getByRole("button", { name: "送る" }));

    const confirm = screen.getByRole("group", { name: "送る前の確認" });
    expect(confirm).toHaveTextContent("次の内容で生徒全員に送りますか？送るとすぐに生徒に届き、プッシュ通知は取り消せません。");
    expect(within(confirm).getByText("年末年始の営業")).toBeInTheDocument();
    expect(sendAnnouncementDraft).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "送るのを確定" }));

    expect(await screen.findByRole("status")).toHaveTextContent(
      "お知らせ「年末年始の営業」を 120 人の生徒に送りました。",
    );
    expect(updateAnnouncementDraft).toHaveBeenCalledWith("eeeeeeee-0000-4000-8000-000000000002", {
      title: "年末年始の営業",
      body: "12月29日から1月3日はお休みです。",
      sport: null,
    });
    expect(sendAnnouncementDraft).toHaveBeenCalledWith("eeeeeeee-0000-4000-8000-000000000002");
    expect(updateAnnouncementDraft.mock.invocationCallOrder[0]).toBeLessThan(
      sendAnnouncementDraft.mock.invocationCallOrder[0]!,
    );
    expect(sendAnnouncement).not.toHaveBeenCalled();
    expect(draftItems().map((item) => item.querySelector("p")?.textContent)).toEqual(["（題なし）"]);
    expect(items()[0]).toHaveTextContent("年末年始の営業");
    expect(items()).toHaveLength(3);
    expect(screen.getByLabelText("題")).toHaveValue("");
    expect(screen.queryByText(/を開いています/)).not.toBeInTheDocument();
  });

  test("題が空の下書きを「送る」と確認に進まず、項目の下に理由を出す", async () => {
    const user = userEvent.setup();
    const sendAnnouncementDraft = sendDraftOk();
    render(<Announcements api={apiOf({ listAnnouncementDrafts: draftListOf(DRAFTS), sendAnnouncementDraft })} />);
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.click(within(draftItems()[1]!).getByRole("button", { name: "送る" }));

    expect(screen.queryByRole("group", { name: "送る前の確認" })).not.toBeInTheDocument();
    expect(screen.getByText("題を入力してください。")).toBeInTheDocument();
    expect(screen.getByLabelText("本文")).toHaveValue("テニスの新しいコーチの紹介（書きかけ）");
    expect(sendAnnouncementDraft).not.toHaveBeenCalled();
  });

  test("下書きから送って対象の生徒がいない（404）ときは、その旨を出して下書きと入力を残す", async () => {
    const user = userEvent.setup();
    const sendAnnouncementDraft = vi.fn<HeadquartersApi["sendAnnouncementDraft"]>(async () => ({
      ok: false,
      code: "not_found",
      message: "対象が見つかりません（ID を確かめてください）。",
    }));
    render(
      <Announcements
        api={apiOf({
          listAnnouncementDrafts: draftListOf(DRAFTS),
          updateAnnouncementDraft: updateOk(),
          sendAnnouncementDraft,
        })}
      />,
    );
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.click(within(draftItems()[0]!).getByRole("button", { name: "送る" }));
    await user.click(screen.getByRole("button", { name: "送るのを確定" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "対象の生徒がいないため、お知らせは送られませんでした。対象を確かめてください。",
    );
    expect(draftItems()).toHaveLength(2);
    expect(screen.getByLabelText("題")).toHaveValue("年末年始の営業");
    expect(screen.getByText(/を開いています/)).toBeInTheDocument();
  });

  test("開いた下書きがもうない（直すときの 404）ときは、その旨を出して一覧から外し、次の保存は新しい下書きにする", async () => {
    const user = userEvent.setup();
    const updateAnnouncementDraft = vi.fn<HeadquartersApi["updateAnnouncementDraft"]>(async () => ({
      ok: false,
      code: "not_found",
      message: "対象が見つかりません（ID を確かめてください）。",
    }));
    render(<Announcements api={apiOf({ listAnnouncementDrafts: draftListOf(DRAFTS), updateAnnouncementDraft })} />);
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.click(within(draftItems()[0]!).getByRole("button", { name: "開いて直す" }));
    await user.click(screen.getByRole("button", { name: "下書きに保存" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "この下書きは見つかりません。ほかの本部のアカウントが送ったか消した可能性があります。",
    );
    expect(draftItems()).toHaveLength(1);
    expect(screen.getByLabelText("題")).toHaveValue("年末年始の営業");
    expect(screen.queryByText(/を開いています/)).not.toBeInTheDocument();
  });

  test("「消す」は行の中で確かめ、「やめる」で戻る。確定で消して残った一覧にし、開いていた下書きなら入力を空にする", async () => {
    const user = userEvent.setup();
    const deleteAnnouncementDraft = vi.fn<HeadquartersApi["deleteAnnouncementDraft"]>(async () => ({
      ok: true,
      data: { announcement_drafts: [DRAFTS[1]!] },
    }));
    render(<Announcements api={apiOf({ listAnnouncementDrafts: draftListOf(DRAFTS), deleteAnnouncementDraft })} />);
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.click(within(draftItems()[0]!).getByRole("button", { name: "開いて直す" }));
    await user.click(within(draftItems()[0]!).getByRole("button", { name: "消す" }));
    const confirm = within(draftItems()[0]!).getByRole("group", { name: "下書きを消す前の確認" });
    expect(confirm).toHaveTextContent("この下書きを消しますか？消した下書きは戻せません。");
    await user.click(within(confirm).getByRole("button", { name: "やめる" }));
    expect(deleteAnnouncementDraft).not.toHaveBeenCalled();

    await user.click(within(draftItems()[0]!).getByRole("button", { name: "消す" }));
    await user.click(within(draftItems()[0]!).getByRole("button", { name: "消すのを確定" }));

    expect(await screen.findByRole("status")).toHaveTextContent("下書き「年末年始の営業」を消しました。");
    expect(deleteAnnouncementDraft).toHaveBeenCalledWith("eeeeeeee-0000-4000-8000-000000000002");
    expect(draftItems().map((item) => item.querySelector("p")?.textContent)).toEqual(["（題なし）"]);
    expect(screen.getByLabelText("題")).toHaveValue("");
    expect(screen.queryByText(/を開いています/)).not.toBeInTheDocument();
  });

  test("消すのに失敗したらその文言を出し、一覧は変えない", async () => {
    const user = userEvent.setup();
    const deleteAnnouncementDraft = vi.fn<HeadquartersApi["deleteAnnouncementDraft"]>(async () => ({
      ok: false,
      message: "サーバーに接続できません。",
    }));
    render(<Announcements api={apiOf({ listAnnouncementDrafts: draftListOf(DRAFTS), deleteAnnouncementDraft })} />);
    await screen.findByRole("list", { name: "下書きの一覧" });

    await user.click(within(draftItems()[1]!).getByRole("button", { name: "消す" }));
    await user.click(screen.getByRole("button", { name: "消すのを確定" }));

    const region = screen.getByRole("region", { name: "下書き" });
    expect(await within(region).findByRole("alert")).toHaveTextContent("サーバーに接続できません。");
    expect(draftItems()).toHaveLength(2);
  });
});

describe("画面への配置", () => {
  test("本部の画面からお知らせの画面へ移れる", () => {
    render(<HeadquartersHome />);

    expect(screen.getByRole("link", { name: "生徒にお知らせを送る" })).toHaveAttribute(
      "href",
      "/headquarters/announcements",
    );
  });

  test("本部以外のロールでは画面に入れず、ログイン画面へ戻す", () => {
    saveSession({ role: "coach", token: "mock.coach.x" });
    render(
      <RoleGate role="headquarters">
        {(session) => <AnnouncementsHome token={session.token} />}
      </RoleGate>,
    );

    expect(screen.queryByRole("region", { name: "お知らせを送る" })).not.toBeInTheDocument();
    expect(replace).toHaveBeenCalledWith("/");
  });
});
