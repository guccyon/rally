import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { createApiClient, type Schemas } from "@/api/client";
import { type CoachApi, coachApi as createCoachApi } from "@/api/coach";
import { LiveSessionPanel } from "@/components/coach/LiveSessionPanel";
import type { LocalMedia } from "@/components/coach/localMedia";
import {
  canOpenPrejoinCheck,
  formatPrejoinOpensAt,
  PREJOIN_CHECK_LEAD_MINUTES,
} from "@/components/coach/prejoinWindow";

import { fakeLocalMedia, type FakeMediaOptions } from "./localMediaFake";

// コーチの入室前チェック（specs/stories/coach-prejoin-check.yaml・specs/ux/issue-1040.yaml の coachPrejoinCheck）。
// カメラ・マイク・スピーカーは作り物（localMediaFake.ts）、時刻は Date と setInterval だけを進める。

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const STUDENT_ID = "33333333-3333-4333-8333-333333333333";
const BOOKING_ID = "44444444-4444-4444-8444-444444444444";
const SESSION_ID = "55555555-5555-4555-8555-555555555555";

// 開始は 2026-11-08 12:00（日本時間）。入室前チェックは 11:55 から。
const STARTS_AT = "2026-11-08T03:00:00Z";
const MINUTE = 60_000;
const startsIn = (minutes: number) => new Date(new Date(STARTS_AT).getTime() - minutes * MINUTE);

const BOOKING: Schemas["CoachBooking"] = {
  booking: {
    booking_id: BOOKING_ID,
    slot_id: "11111111-1111-4111-8111-111111111111",
    student_id: STUDENT_ID,
    coach_id: COACH_ID,
    is_trial: false,
    status: "confirmed",
    starts_at: STARTS_AT,
    created_at: "2026-11-01T00:00:00Z",
    confirmed_at: "2026-11-01T01:00:00Z",
    cancelled_at: null,
    lesson_menu: {
      lesson_menu_id: "a0000000-0000-4000-8000-000000000001",
      name: "フォアハンドの基礎",
      duration_minutes: 30,
    },
  },
  slot: {
    slot_id: "11111111-1111-4111-8111-111111111111",
    coach_id: COACH_ID,
    sport: "tennis",
    starts_at: STARTS_AT,
    ends_at: "2026-11-08T03:30:00Z",
    status: "booked",
    capacity: 1,
    remaining_seats: 0,
  },
  student_display_name: "鈴木 一郎",
};

const STARTED: Schemas["LiveLessonSession"] = {
  session_id: SESSION_ID,
  booking_id: BOOKING_ID,
  student_id: STUDENT_ID,
  coach_id: COACH_ID,
  status: "in_progress",
  started_at: STARTS_AT,
  ended_at: null,
  duration_sec: null,
  room_token: "mock-room",
};

function api(start?: CoachApi["startLiveLessonSession"]) {
  return {
    listMyCoachBookings: vi.fn<CoachApi["listMyCoachBookings"]>(async () => ({
      ok: true,
      data: { bookings: [BOOKING] },
    })),
    startLiveLessonSession: vi.fn<CoachApi["startLiveLessonSession"]>(
      start ?? (async () => ({ ok: true, data: STARTED })),
    ),
    endLiveLessonSession: vi.fn<CoachApi["endLiveLessonSession"]>(),
  };
}

/** 時刻を進め、1 秒ごとの書き換え（残り時間）と音の大きさの書き換えを流す。 */
function advance(ms: number) {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

/** 今を開始の minutes 分前にして一覧を開き、その予約の入室前チェックを開く。 */
async function openCheck(
  options: FakeMediaOptions = {},
  minutes = 3,
  start?: CoachApi["startLiveLessonSession"],
  override?: (media: LocalMedia) => LocalMedia,
) {
  vi.setSystemTime(startsIn(minutes));
  const fake = fakeLocalMedia(options);
  const coachApi = api(start);
  const user = userEvent.setup();
  const view = render(<LiveSessionPanel api={coachApi} media={override ? override(fake.media) : fake.media} />);
  const row = await screen.findByRole("row", { name: /鈴木 一郎/ });
  await user.click(within(row).getByRole("button", { name: "入室前チェック" }));
  await screen.findByRole("heading", { name: "入室前チェック" });
  return { fake, coachApi, user, view };
}

beforeEach(() => {
  // findBy・userEvent の待ち（setTimeout）は本物のまま、時刻と間隔の書き換えだけを進める
  vi.useFakeTimers({ toFake: ["Date", "setInterval", "clearInterval"] });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("開ける時刻と 5 分の定数", () => {
  test("5 分は 1 か所の定数で、開始 5 分前ちょうどから開ける", () => {
    expect(PREJOIN_CHECK_LEAD_MINUTES).toBe(5);
    expect(canOpenPrejoinCheck(STARTS_AT, startsIn(5).getTime() - 1)).toBe(false);
    expect(canOpenPrejoinCheck(STARTS_AT, startsIn(5).getTime())).toBe(true);
  });

  test("開ける時刻は今日なら時刻だけ、ほかの日なら月日と曜日を添える", () => {
    expect(formatPrejoinOpensAt(STARTS_AT, startsIn(60).getTime())).toBe("11:55");
    expect(formatPrejoinOpensAt(STARTS_AT, startsIn(24 * 60).getTime())).toBe("11/8（日）11:55");
  });
});

describe("通話への参加の一覧", () => {
  test("開始 5 分前より前は開ける時刻を出してチェックを開かず、開いたまま 5 分前になると読み込み直さずに開ける", async () => {
    vi.setSystemTime(startsIn(6));
    const coachApi = api();
    render(<LiveSessionPanel api={coachApi} media={fakeLocalMedia().media} />);

    const row = await screen.findByRole("row", { name: /鈴木 一郎/ });
    expect(row).toHaveTextContent("開始 5 分前から入室前チェックができます（11:55 から）");
    expect(within(row).queryByRole("button")).not.toBeInTheDocument();

    advance(MINUTE);

    expect(within(row).getByRole("button", { name: "入室前チェック" })).toBeEnabled();
    expect(coachApi.listMyCoachBookings).toHaveBeenCalledTimes(1);
  });
});

describe("入室前チェック", () => {
  test("開くと予約の要約・本物のカメラの映り・マイクの音の大きさを出し、開始時刻の前は入室できない", async () => {
    const { fake, coachApi } = await openCheck({ level: 0.5 });

    expect(screen.getByText(/生徒: 鈴木 一郎・メニュー: フォアハンドの基礎（30 分）/)).toBeInTheDocument();
    expect(await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" })).toBeInTheDocument();
    expect(fake.live().map((stream) => stream.kind).sort()).toEqual(["camera", "mic"]);
    expect(screen.getByRole("button", { name: "マイクを切る" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "カメラを切る" })).toBeEnabled();

    advance(200);
    const meter = screen.getByTestId("level-meter");
    expect(meter.querySelectorAll("[data-reached]")).toHaveLength(5);
    expect(screen.getByText("音が届いています")).toBeInTheDocument();

    const bar = screen.getByRole("region", { name: "入室" });
    expect(bar).toHaveTextContent("12:00");
    expect(bar).toHaveTextContent("開始時刻になると入室できます");
    expect(within(bar).getByRole("button", { name: "入室する" })).toBeDisabled();
    expect(coachApi.startLiveLessonSession).not.toHaveBeenCalled();
  });

  test("開いたまま開始時刻を過ぎると読み込み直さずに入室でき、入室したらカメラとマイクを止めて入・切を引き継ぐ", async () => {
    const { fake, coachApi, user } = await openCheck();
    await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" });
    await user.click(screen.getByRole("button", { name: "マイクを切る" }));
    expect(screen.getByText("マイクを切っています")).toBeInTheDocument();
    expect(fake.live().map((stream) => stream.kind)).toEqual(["camera"]);

    const bar = screen.getByRole("region", { name: "入室" });
    advance(3 * MINUTE - 1000);
    expect(within(bar).getByRole("button", { name: "入室する" })).toBeDisabled();
    advance(1000);
    expect(within(bar).getByRole("button", { name: "入室する" })).toBeEnabled();
    expect(bar).toHaveTextContent("入室すると、鈴木 一郎さんとのビデオ通話が始まります。");

    await user.click(within(bar).getByRole("button", { name: "入室する" }));

    expect(coachApi.startLiveLessonSession).toHaveBeenCalledWith({ booking_id: BOOKING_ID });
    expect(await screen.findByText("通話中")).toBeInTheDocument();
    expect(screen.getByText("通話を開始しました。")).toBeInTheDocument();
    expect(screen.getByText("カメラ: 入・マイク: 切（入室前チェックのまま）")).toBeInTheDocument();
    expect(fake.live()).toEqual([]);
    expect(fake.probes.every((probe) => probe.closed)).toBe(true);
  });

  test("入室すると一覧と入れ替わって通話中の画面になり、見出しに焦点が移って先頭を見せる。終了すると「通話を終了しました。」を出したまま一覧に戻る", async () => {
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    try {
      const { coachApi, user } = await openCheck({}, 0);
      coachApi.endLiveLessonSession.mockResolvedValue({
        ok: true,
        data: { ...STARTED, status: "ended", ended_at: "2026-11-08T03:25:00Z", duration_sec: 1500 },
      });
      await user.click(screen.getByRole("button", { name: "入室する" }));

      // 長い一覧の下に埋もれないよう、一覧の表は出さず、通話中の画面だけを出す
      const heading = await screen.findByRole("heading", { name: "ビデオ通話中" });
      expect(screen.queryByRole("table")).not.toBeInTheDocument();
      expect(screen.getByRole("region", { name: "ビデオ通話中" })).toHaveTextContent("生徒: 鈴木 一郎");
      expect(screen.getByRole("status")).toHaveTextContent("通話を開始しました。");
      expect(screen.getByText("通話中")).toBeInTheDocument();
      expect(screen.getByText("カメラ: 入・マイク: 入（入室前チェックのまま）")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "通話を終了" })).toBeEnabled();
      expect(heading).toHaveFocus();
      expect(scrollIntoView).toHaveBeenCalledWith({ block: "start" });
      expect(scrollIntoView.mock.contexts.at(-1)).toBe(heading);

      await user.click(screen.getByRole("button", { name: "通話を終了" }));

      expect(coachApi.endLiveLessonSession).toHaveBeenCalledWith(SESSION_ID);
      expect(await screen.findByRole("table", { name: "これからの予約" })).toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: "ビデオ通話中" })).not.toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent("通話を終了しました。");
      expect(screen.getByText("1500 秒")).toBeInTheDocument();
      // 押したボタンが消えても焦点は迷子にならず、一覧の先頭に移る
      expect(document.activeElement).not.toBe(document.body);
      expect(document.activeElement).toContainElement(screen.getByRole("status"));
    } finally {
      delete (Element.prototype as Partial<Element>).scrollIntoView;
    }
  });

  test("終了に失敗したら通話中の画面のまま理由を出し、もう一度終了できる", async () => {
    const { coachApi, user } = await openCheck({}, 0);
    coachApi.endLiveLessonSession.mockResolvedValue({ ok: false, message: "通話を終了できませんでした。" });
    await user.click(screen.getByRole("button", { name: "入室する" }));
    await screen.findByRole("heading", { name: "ビデオ通話中" });

    await user.click(screen.getByRole("button", { name: "通話を終了" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("通話を終了できませんでした。");
    expect(screen.getByRole("heading", { name: "ビデオ通話中" })).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "通話を終了" })).toBeEnabled();
  });

  test("入室で 409 slot_not_started が返ったら今の文言を出し、チェックは開いたまま", async () => {
    const message =
      "この予約の開始時刻になっていないため、ビデオ通話を始められません。開始時刻になってから、もう一度始めてください。";
    const { fake, user } = await openCheck({}, 0, async () => ({ ok: false, code: "slot_not_started", message }));
    await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" });

    await user.click(screen.getByRole("button", { name: "入室する" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(message);
    expect(screen.getByRole("heading", { name: "入室前チェック" })).toBeInTheDocument();
    expect(fake.live()).toHaveLength(2);
  });

  test("カメラを切ると流れを止めて枠に切った旨を出し、入れ直すと映る", async () => {
    const { fake, user } = await openCheck();
    await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" });

    await user.click(screen.getByRole("button", { name: "カメラを切る" }));
    expect(screen.getByText("カメラを切っています")).toBeInTheDocument();
    expect(fake.live().map((stream) => stream.kind)).toEqual(["mic"]);

    await user.click(screen.getByRole("button", { name: "カメラを入れる" }));
    expect(await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" })).toBeInTheDocument();
  });

  test("「スピーカーを試す」で試し音を鳴らし、もう一度押すと止める。鳴らせなければその旨を出す", async () => {
    const { fake, user } = await openCheck({ sound: "manual" });

    await user.click(screen.getByRole("button", { name: "スピーカーを試す" }));
    expect(fake.media.playTestSound).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/試し音を鳴らしています。/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "試し音を止める" }));
    expect(fake.sounds[0]!.stopped).toBe(true);
    expect(await screen.findByRole("button", { name: "スピーカーを試す" })).toBeInTheDocument();
  });

  test("試し音を鳴らせなかったら、音量と出る先を確かめる旨を出す", async () => {
    const { user } = await openCheck({ sound: "failed" });

    await user.click(screen.getByRole("button", { name: "スピーカーを試す" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "試し音を鳴らせませんでした。PC の音量と、音の出る先を確かめてください。",
    );
  });

  test("カメラ・マイク・スピーカーが 1 つずつなら、使う機器の欄を出さない", async () => {
    await openCheck();
    await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" });
    expect(screen.queryByRole("region", { name: "使う機器" })).not.toBeInTheDocument();
  });

  test("2 つ以上あるカメラ・マイク・スピーカーを選べ、選ぶとすぐにその機器に切り替える", async () => {
    const { fake, user } = await openCheck({
      cameras: [
        { deviceId: "cam-1", label: "内蔵カメラ" },
        { deviceId: "cam-2", label: "外付けカメラ" },
      ],
      mics: [
        { deviceId: "mic-1", label: "内蔵マイク" },
        { deviceId: "mic-2", label: "ヘッドセットのマイク" },
      ],
      speakers: [
        { deviceId: "spk-1", label: "内蔵スピーカー" },
        { deviceId: "spk-2", label: "ヘッドセット" },
      ],
      sound: "manual",
    });
    const devices = await screen.findByRole("region", { name: "使う機器" });

    await user.selectOptions(within(devices).getByRole("combobox", { name: "カメラ" }), "外付けカメラ");
    expect(fake.media.open).toHaveBeenLastCalledWith("camera", "cam-2");
    expect(fake.live().find((stream) => stream.kind === "camera")?.deviceId).toBe("cam-2");
    expect(fake.live()).toHaveLength(2);

    await user.selectOptions(within(devices).getByRole("combobox", { name: "マイク" }), "ヘッドセットのマイク");
    expect(fake.live().find((stream) => stream.kind === "mic")?.deviceId).toBe("mic-2");

    await user.selectOptions(within(devices).getByRole("combobox", { name: "スピーカー" }), "ヘッドセット");
    await user.click(screen.getByRole("button", { name: "スピーカーを試す" }));
    expect(fake.media.playTestSound).toHaveBeenLastCalledWith("spk-2");
  });

  test("選んだカメラを使えなければ、その旨を出して前のカメラのまま", async () => {
    const { fake, user } = await openCheck({
      cameras: [
        { deviceId: "cam-1", label: "内蔵カメラ" },
        { deviceId: "cam-2", label: "外付けカメラ" },
      ],
    });
    fake.state.broken.add("cam-2");
    const devices = await screen.findByRole("region", { name: "使う機器" });

    await user.selectOptions(within(devices).getByRole("combobox", { name: "カメラ" }), "外付けカメラ");

    expect(
      await within(devices).findByText("このカメラを使えませんでした。ほかのアプリが使っていないか確かめてください。"),
    ).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "自分のカメラの映り（左右反転）" })).toBeInTheDocument();
    expect(fake.live().filter((stream) => stream.kind === "camera")).toHaveLength(1);
  });

  test("使っていたカメラが外れたら、既定の機器に戻してその旨を出す", async () => {
    const { fake, user } = await openCheck({
      cameras: [
        { deviceId: "cam-1", label: "内蔵カメラ" },
        { deviceId: "cam-2", label: "外付けカメラ" },
        { deviceId: "cam-3", label: "書画カメラ" },
      ],
    });
    const devices = await screen.findByRole("region", { name: "使う機器" });
    await user.selectOptions(within(devices).getByRole("combobox", { name: "カメラ" }), "外付けカメラ");

    fake.state.cameras = fake.state.cameras.filter((device) => device.deviceId !== "cam-2");
    act(() => fake.changeDevices());

    expect(
      await within(devices).findByText("外付けカメラ が外れたため、既定の機器に切り替えました。"),
    ).toBeInTheDocument();
    expect(fake.live().filter((stream) => stream.kind === "camera").map((stream) => stream.deviceId)).toEqual(["cam-1"]);
  });

  test("許可を断られたら直し方を出し、開始時刻から入室できる。許可して確かめ直すと映る", async () => {
    const { fake, user } = await openCheck({ camera: "denied", mic: "denied" }, 0);

    expect(
      await screen.findByText("カメラとマイクが使えません。ブラウザーの設定で許可すると確かめられます。"),
    ).toBeInTheDocument();
    expect(screen.getByText(/許可しなくても、開始時刻から入室できます。/)).toBeInTheDocument();
    expect(screen.getByText("カメラが使えません")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "マイクを切る" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "カメラを切る" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "スピーカーを試す" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "入室する" })).toBeEnabled();

    fake.state.camera = "ok";
    fake.state.mic = "ok";
    await user.click(screen.getByRole("button", { name: "もう一度確かめる" }));

    expect(await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" })).toBeInTheDocument();
    expect(screen.queryByText(/ブラウザーの設定で許可すると確かめられます。/)).not.toBeInTheDocument();
  });

  test("カメラだけ断られたら、カメラが使えない旨だけを出し、マイクは使える", async () => {
    await openCheck({ camera: "denied" });

    expect(
      await screen.findByText("カメラが使えません。ブラウザーの設定で許可すると確かめられます。"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "マイクを切る" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "カメラを切る" })).not.toBeInTheDocument();
  });

  test("カメラがないときはその旨を出し、カメラの切り替えを出さず、入室は止めない", async () => {
    await openCheck({ camera: "none" }, 0);

    expect(
      await screen.findByText("この PC にカメラが見つかりません。マイクとスピーカーは確かめられます。"),
    ).toBeInTheDocument();
    expect(screen.getByText("カメラが見つかりません")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^カメラを/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "マイクを切る" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "入室する" })).toBeEnabled();
  });

  test("マイクの許可を断られたら、音の大きさのかわりに直し方を出す", async () => {
    await openCheck({ mic: "denied" });

    expect(
      await screen.findByText("マイクが使えないため、音の大きさを出せません。ブラウザーの設定で許可すると確かめられます。"),
    ).toBeInTheDocument();
  });

  test("「チェックをやめる」で一覧に戻り、カメラ・マイク・試し音を止める", async () => {
    const { fake, user } = await openCheck({ sound: "manual" });
    await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" });
    await user.click(screen.getByRole("button", { name: "スピーカーを試す" }));
    advance(200);

    await user.click(screen.getByRole("button", { name: "チェックをやめる" }));

    expect(await screen.findByRole("table", { name: "これからの予約" })).toBeInTheDocument();
    expect(fake.live()).toEqual([]);
    expect(fake.probes.every((probe) => probe.closed)).toBe(true);
    expect(fake.sounds[0]!.stopped).toBe(true);
  });

  test("「通話への参加に戻る」でも一覧に戻り、カメラとマイクを止める", async () => {
    const { fake, user } = await openCheck();
    await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" });

    await user.click(screen.getByRole("button", { name: "通話への参加に戻る" }));

    expect(await screen.findByRole("table", { name: "これからの予約" })).toBeInTheDocument();
    expect(fake.live()).toEqual([]);
  });

  test("画面を離れたら（タブの切り替え）、許可を待っている途中のカメラとマイクも開いたらすぐ止める", async () => {
    const pending: (() => void)[] = [];
    const { fake, view } = await openCheck({}, 3, undefined, (media) => ({
      ...media,
      // 許可のダイアログに答えるまで開かない
      open: (kind, deviceId) =>
        new Promise((resolve) => pending.push(() => void media.open(kind, deviceId).then(resolve))),
    }));
    expect(screen.getByText("カメラの許可を待っています")).toBeInTheDocument();

    view.unmount();
    await act(async () => {
      for (const answer of pending) answer();
    });

    expect(fake.streams).toHaveLength(2);
    expect(fake.live()).toEqual([]);
  });
});

describe("サーバーに合わせた時計", () => {
  /**
   * 本物の API クライアント（call.ts の Date ヘッダーの読み取りまで通す）。サーバーの時計は端末の時計 + skew()。
   * skew() が null なら応答に Date を付けない。入室の応答は start で決める（既定は 201）。
   */
  function serverApi(skew: () => number | null, start?: () => { status: number; body: unknown }) {
    const fetchMock = vi.fn(async (input: Request) => {
      const reply =
        input.method === "POST" && new URL(input.url).pathname === "/api/live-lesson-sessions"
          ? (start?.() ?? { status: 201, body: STARTED })
          : { status: 200, body: { bookings: [BOOKING] } };
      const headers: Record<string, string> = { "content-type": "application/json" };
      const current = skew();
      if (current !== null) headers.Date = new Date(Date.now() + current).toUTCString();
      return new Response(JSON.stringify(reply.body), { status: reply.status, headers });
    });
    const client = createApiClient("mock.coach.abc", {
      baseUrl: "http://localhost/api",
      fetch: (input) => fetchMock(input as Request),
    });
    return { api: createCoachApi(client), fetchMock };
  }

  /** 端末の時計を開始の deviceMinutes 分前にして一覧を開き、入室前チェックを開く。 */
  async function openWithServer(deviceMinutes: number, server: ReturnType<typeof serverApi>) {
    vi.setSystemTime(startsIn(deviceMinutes));
    const user = userEvent.setup();
    render(<LiveSessionPanel api={server.api} media={fakeLocalMedia().media} />);
    const row = await screen.findByRole("row", { name: /鈴木 一郎/ });
    await user.click(within(row).getByRole("button", { name: "入室前チェック" }));
    await screen.findByRole("img", { name: "自分のカメラの映り（左右反転）" });
    return { user, bar: screen.getByRole("region", { name: "入室" }) };
  }

  const joinButton = (bar: HTMLElement) => within(bar).getByRole("button", { name: "入室する" });
  const startCalls = (fetchMock: ReturnType<typeof serverApi>["fetchMock"]) =>
    fetchMock.mock.calls.filter(([request]) => request.method === "POST");

  test("PC の時計が 2 分遅れていても、サーバーの開始時刻ちょうどに入室でき、残り時間もサーバーの時刻で数える", async () => {
    // サーバーは開始 3 分前、PC は開始 5 分前
    const server = serverApi(() => 2 * MINUTE);
    const { user, bar } = await openWithServer(5, server);

    expect(bar).toHaveTextContent("あと 3:00");
    advance(3 * MINUTE - 1000);
    expect(joinButton(bar)).toBeDisabled();
    advance(1000);
    expect(joinButton(bar)).toBeEnabled();

    await user.click(joinButton(bar));
    expect(await screen.findByText("通話中")).toBeInTheDocument();
  });

  test("PC の時計が 2 分進んでいても、PC の開始時刻では押せず、サーバーの開始時刻ちょうどに押せる", async () => {
    // サーバーは開始 3 分前、PC は開始 1 分前
    const server = serverApi(() => -2 * MINUTE);
    const { bar } = await openWithServer(1, server);

    expect(bar).toHaveTextContent("あと 3:00");
    advance(MINUTE);
    expect(joinButton(bar)).toBeDisabled();
    advance(2 * MINUTE - 1000);
    expect(joinButton(bar)).toBeDisabled();
    advance(1000);
    expect(joinButton(bar)).toBeEnabled();
    expect(startCalls(server.fetchMock)).toHaveLength(0);
  });

  test("入室で 409 slot_not_started が返ったら、その応答の Date で合わせ直して残り時間の表示に戻り、入室を繰り返し送らない", async () => {
    // 一覧の応答に Date がなく PC の時計（開始時刻ちょうど）のまま。サーバーは本当は開始 2 分前で、入室の応答で分かる
    let skew: number | null = null;
    const server = serverApi(
      () => skew,
      () => {
        skew = -2 * MINUTE;
        return { status: 409, body: { code: "slot_not_started", message: "予約したスロットの開始時刻より前" } };
      },
    );
    const fetchMock = server.fetchMock;
    const { user, bar } = await openWithServer(0, server);
    expect(joinButton(bar)).toBeEnabled();

    await user.click(joinButton(bar));

    expect(await screen.findByRole("alert")).toHaveTextContent("この予約の開始時刻になっていないため");
    expect(joinButton(bar)).toBeDisabled();
    expect(bar).toHaveTextContent("あと 2:00");
    expect(startCalls(fetchMock)).toHaveLength(1);

    advance(2 * MINUTE);
    expect(joinButton(bar)).toBeEnabled();
    expect(startCalls(fetchMock)).toHaveLength(1);
  });

  test("応答に Date がなければ PC の時計のまま判断し、画面は止まらない", async () => {
    const server = serverApi(() => null);
    const { bar } = await openWithServer(1, server);

    expect(bar).toHaveTextContent("あと 1:00");
    advance(MINUTE - 1000);
    expect(joinButton(bar)).toBeDisabled();
    advance(1000);
    expect(joinButton(bar)).toBeEnabled();
  });
});
