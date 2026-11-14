import type { Schemas } from "@/api/client";

// 画面のキャプチャ用の決まった応答。型は契約から生成した Schemas だけを使い、契約が変われば typecheck で気づく。
// 時刻は「今」（NOW）を基準に並べる。今は 2026-10-26 12:00（日本時間）に固定する。

export const NOW = new Date("2026-10-26T03:00:00Z");

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const COACH_ID_2 = "22222222-2222-4222-8222-222222222223";
const STUDENT_ID = "11111111-1111-4111-8111-111111111111";
const STUDENT_ID_2 = "11111111-1111-4111-8111-111111111112";
const THREAD_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const HEADQUARTERS_ID = "44444444-4444-4444-8444-444444444444";

/** NOW から hours 時間後（負なら前）の時刻。 */
function at(hours: number): string {
  return new Date(NOW.getTime() + hours * 3_600_000).toISOString();
}

function id(prefix: string, n: number): string {
  return `${prefix}${String(n).padStart(4, "0")}-0000-4000-8000-000000000000`;
}

const MENU_FORE: Schemas["LessonMenu"] = {
  lesson_menu_id: id("aaaa", 1),
  coach_id: COACH_ID,
  sport: "tennis",
  lesson_type: "live_lesson",
  name: "フォアハンドの基礎",
  description: "グリップと打点を確かめながら、安定したフォアハンドを身につけます。",
  duration_minutes: 60,
  is_published: true,
  created_at: at(-24 * 30),
};

const MENU_SERVE: Schemas["LessonMenu"] = {
  ...MENU_FORE,
  lesson_menu_id: id("aaaa", 2),
  name: "サーブの添削",
  description: "送ってもらったサーブの動画を見て、改善点を返します。",
  lesson_type: "async_lesson",
  duration_minutes: null,
  created_at: at(-24 * 20),
};

const MENU_VOLLEY: Schemas["LessonMenu"] = {
  ...MENU_FORE,
  lesson_menu_id: id("aaaa", 3),
  name: "ボレーの練習",
  description: "",
  duration_minutes: 30,
  is_published: false,
  created_at: at(-24 * 10),
};

function slot(n: number, startHours: number, values: Partial<Schemas["Slot"]> = {}): Schemas["Slot"] {
  return {
    slot_id: id("3333", n),
    coach_id: COACH_ID,
    sport: "tennis",
    starts_at: at(startHours),
    ends_at: at(startHours + 1),
    status: "open",
    capacity: 1,
    remaining_seats: 1,
    lesson_menu: null,
    ...values,
  };
}

const SLOTS: Schemas["CoachSlot"][] = [
  {
    slot: slot(1, 24, {
      status: "booked",
      remaining_seats: 0,
      lesson_menu: { lesson_menu_id: MENU_FORE.lesson_menu_id, name: MENU_FORE.name, duration_minutes: 60 },
    }),
    has_active_booking: true,
  },
  { slot: slot(2, 48, { capacity: 4, remaining_seats: 3 }), has_active_booking: true },
  { slot: slot(3, 72), has_active_booking: false },
  { slot: slot(4, -48, { status: "booked", remaining_seats: 0 }), has_active_booking: true },
];

function booking(
  n: number,
  s: Schemas["Slot"],
  status: Schemas["BookingStatus"],
  student: { id: string; name: string },
): Schemas["CoachBooking"] {
  return {
    booking: {
      booking_id: id("4444", n),
      slot_id: s.slot_id,
      student_id: student.id,
      coach_id: COACH_ID,
      is_trial: n === 1,
      status,
      starts_at: s.starts_at,
      created_at: at(-24 * 3),
      confirmed_at: status === "confirmed" ? at(-24 * 2) : null,
      cancelled_at: null,
      lesson_menu: s.lesson_menu
        ? { lesson_menu_id: s.lesson_menu.lesson_menu_id, name: s.lesson_menu.name, duration_minutes: 60 }
        : null,
    },
    slot: s,
    student_display_name: student.name,
  };
}

const YAMADA = { id: STUDENT_ID, name: "山田 花子" };
const SATO = { id: STUDENT_ID_2, name: "佐藤 一郎" };

/** 入室前チェックを撮る予約の開始（今の 3 分後。12:03 開始で、11:58 から入室前チェックを開ける）。 */
export const PREJOIN_STARTS_AT = new Date(NOW.getTime() + 3 * 60_000);

const BOOKINGS: Schemas["CoachBooking"][] = [
  booking(1, SLOTS[0]!.slot, "confirmed", YAMADA),
  booking(2, SLOTS[1]!.slot, "pending", SATO),
  booking(3, SLOTS[3]!.slot, "confirmed", YAMADA),
  // 入室前チェック（coach-prejoin-check*）。本部の予約の一覧（HQ_BOOKINGS）には入れない
  booking(
    4,
    slot(5, 0.05, {
      status: "booked",
      remaining_seats: 0,
      ends_at: at(0.55),
      lesson_menu: { lesson_menu_id: MENU_FORE.lesson_menu_id, name: MENU_FORE.name, duration_minutes: 30 },
    }),
    "confirmed",
    SATO,
  ),
];

/** 入室前チェックから入室したときの通話セッション（coach-live-in-call。入室前チェックの予約に、開始時刻ちょうどに入った）。 */
export const PREJOIN_STARTED_SESSION: Schemas["LiveLessonSession"] = {
  session_id: id("5555", 1),
  booking_id: BOOKINGS[3]!.booking.booking_id,
  student_id: SATO.id,
  coach_id: COACH_ID,
  status: "in_progress",
  started_at: PREJOIN_STARTS_AT.toISOString(),
  ended_at: null,
  duration_sec: null,
  room_token: "mock-room",
};

// 本部の予約の一覧（今日から 7 日）。確定待ちのまま 24 時間以内に始まる予約・確定済み・確定待ち・キャンセル済みを混ぜる。
const HQ_BOOKINGS: Schemas["HeadquartersBooking"][] = [
  {
    booking: {
      ...BOOKINGS[0]!.booking,
      booking_id: id("4444", 9),
      slot_id: id("3333", 9),
      coach_id: COACH_ID_2,
      student_id: STUDENT_ID_2,
      status: "pending",
      starts_at: at(5),
      confirmed_at: null,
      lesson_menu: null,
    },
    coach_display_name: "高橋 コーチ",
    student_display_name: SATO.name,
  },
  ...BOOKINGS.slice(0, 2).map((item) => ({
    booking: item.booking,
    coach_display_name: "鈴木 コーチ",
    student_display_name: item.student_display_name,
  })),
  {
    booking: {
      ...BOOKINGS[1]!.booking,
      booking_id: id("4444", 10),
      student_id: STUDENT_ID,
      status: "cancelled",
      starts_at: at(48),
      cancelled_at: at(-2),
    },
    coach_display_name: "鈴木 コーチ",
    student_display_name: YAMADA.name,
  },
];

/**
 * 本部の予約の詳細の状態ごとの記録（確定のあとに続く記録）。枠の開始は startsAt（なければ at(48)）。
 * 確定だけ・生徒のキャンセル（枠を使った）・コーチのお休みのキャンセル・記録のない古いキャンセル。
 */
export const HQ_BOOKING_DETAIL_STATES: {
  bookingId: string;
  startsAt?: string;
  statusChanges: Schemas["BookingStatusChange"][];
}[] = [
  { bookingId: id("4444", 30), statusChanges: [] },
  {
    bookingId: id("4444", 31),
    startsAt: at(15),
    statusChanges: [
      {
        status: "cancelled",
        changed_at: at(-2),
        cancelled_by: "student",
        consumes_quota: true,
        hours_before_start: 17,
        cancel_reason: null,
        coach_leave: null,
      },
    ],
  },
  {
    bookingId: id("4444", 32),
    statusChanges: [
      {
        status: "cancelled",
        changed_at: at(-2),
        cancelled_by: "headquarters",
        consumes_quota: false,
        hours_before_start: 50,
        cancel_reason: "coach_leave",
        coach_leave: {
          coach_leave_id: id("7777", 1),
          coach_id: COACH_ID,
          from: "2026-10-28",
          to: "2026-11-03",
          note: "体調を崩したため、この期間のレッスンをお休みします。ほかのコーチでの予約をお願いします。",
          created_by: HEADQUARTERS_ID,
          created_by_display_name: "佐々木 陽子",
          created_at: at(-2),
        },
      },
    ],
  },
  {
    bookingId: id("4444", 33),
    statusChanges: [
      {
        status: "cancelled",
        changed_at: at(-2),
        cancelled_by: null,
        consumes_quota: null,
        hours_before_start: 50,
        cancel_reason: null,
        coach_leave: null,
      },
    ],
  },
];

const VIDEOS: Schemas["CoachVideo"][] = [
  {
    video_id: id("6666", 1),
    student_display_name: YAMADA.name,
    submitted_at: at(-5),
    review_status: "in_progress",
    ai_feedback: {
      comment: "トスが体より後ろに上がっています。打点が下がり、ネットにかかりやすくなっています。",
      created_at: at(-4.9),
    },
    lesson_menu: { lesson_menu_id: MENU_SERVE.lesson_menu_id, name: MENU_SERVE.name },
  },
  {
    video_id: id("6666", 2),
    student_display_name: SATO.name,
    submitted_at: at(-24 * 4),
    review_status: "returned",
    returned_at: at(-24 * 3),
    ai_feedback: null,
    lesson_menu: null,
  },
];

const MESSAGES: Schemas["Message"][] = [
  {
    message_id: id("dddd", 1),
    thread_id: THREAD_ID,
    sender_id: STUDENT_ID,
    sender_role: "student",
    kind: "text",
    body: "先日はありがとうございました。サーブのトスがまだ安定しません。",
    sent_at: at(-26),
    read_at: at(-25),
  },
  {
    message_id: id("dddd", 2),
    thread_id: THREAD_ID,
    sender_id: COACH_ID,
    sender_role: "coach",
    kind: "text",
    body: "動画を送ってもらえれば、トスの位置を一緒に見ましょう。",
    sent_at: at(-25),
    read_at: at(-24),
  },
  {
    message_id: id("dddd", 3),
    thread_id: THREAD_ID,
    sender_id: STUDENT_ID,
    sender_role: "student",
    kind: "video_review_request",
    body: "添削の依頼（1 回目）",
    video_id: VIDEOS[0]!.video_id,
    sent_at: at(-5),
    read_at: null,
  },
  {
    message_id: id("dddd", 4),
    thread_id: THREAD_ID,
    sender_id: STUDENT_ID,
    sender_role: "student",
    kind: "stamp",
    body: "スタンプ: 🙏 ありがとう",
    stamp_id: "thanks",
    sent_at: at(-4.5),
    read_at: null,
  },
];

const THREADS: Schemas["MessageThread"][] = [
  {
    thread_id: THREAD_ID,
    student_id: STUDENT_ID,
    coach_id: COACH_ID,
    opened_at: at(-24 * 7),
    last_message: MESSAGES[3]!,
    unread_count: 2,
    // 最後の発言が生徒なので未返信
    unanswered: true,
  },
];

function review(n: number, rating: number, comment: string, hoursAgo: number, menu = true): Schemas["CoachReview"] {
  return {
    review_id: id("ffff", n),
    lesson_id: id("7777", n),
    rating,
    comment,
    submitted_at: at(-hoursAgo),
    lesson_menu: menu ? { lesson_menu_id: MENU_FORE.lesson_menu_id, name: MENU_FORE.name } : null,
  };
}

const REVIEWS: Schemas["CoachReview"][] = [
  review(1, 5, "打点の説明がとてもわかりやすかったです。", 20),
  review(2, 4, "楽しく練習できました。", 24 * 5, false),
  {
    ...review(3, 5, "次回もお願いします！", 24 * 12),
    reply: { comment: "ありがとうございます。次回もお待ちしています！", replied_at: at(-24 * 11) },
  },
];

function lowReview(
  n: number,
  rating: number,
  comment: string,
  hoursAgo: number,
  coach: { id: string; name: string },
  student: { id: string; name: string },
  menu = true,
): Schemas["LowRatedLessonReview"] {
  const base = review(n, rating, comment, hoursAgo, menu);
  return {
    ...base,
    coach_id: coach.id,
    coach_display_name: coach.name,
    student_id: student.id,
    student_display_name: student.name,
    lesson_menu: base.lesson_menu ?? null,
  };
}

const SUZUKI_COACH = { id: COACH_ID, name: "鈴木 コーチ" };
const TAKAHASHI_COACH = { id: COACH_ID_2, name: "高橋 コーチ" };

// 終わっていないビデオ通話。経過は NOW 時点の値（開始の古い順）。名前の引けないコーチの行を 1 つ混ぜる。
const OVERDUE_SESSIONS: Schemas["OverdueLiveLessonSession"][] = [
  {
    session_id: id("5555", 1),
    booking_id: id("6666", 1),
    coach_id: COACH_ID,
    coach_display_name: "鈴木 コーチ",
    student_id: STUDENT_ID,
    student_display_name: "山田 花子",
    started_at: at(-26.5),
    elapsed_sec: 26.5 * 3600,
  },
  {
    session_id: id("5555", 2),
    booking_id: id("6666", 2),
    coach_id: COACH_ID_2,
    student_id: STUDENT_ID_2,
    student_display_name: "佐藤 一郎",
    started_at: at(-4.25),
    elapsed_sec: 4.25 * 3600,
  },
];

const LOW_REVIEWS: Schemas["LowRatedLessonReview"][] = [
  lowReview(11, 1, "通話が途中で何度も切れて、ほとんど教えてもらえませんでした。", 6, SUZUKI_COACH, YAMADA),
  lowReview(12, 2, "説明が早口で、何を直せばいいのか分かりにくかったです。", 24 * 2, TAKAHASHI_COACH, SATO, false),
  {
    // 本部が対応済みにした行（対応済みの日時を出し、「対応済みにする」は出ない）。
    ...lowReview(13, 2, "開始が 10 分遅れました。", 24 * 4, SUZUKI_COACH, SATO),
    handling: { handled_by: id("5555", 1), handled_at: at(-24 * 3) },
  },
];

// 作った時刻の新しい順。未読 2 件・既読 1 件（見出しの「お知らせ」のバッジは 2）。
const NOTIFICATIONS: Schemas["Notification"][] = [
  {
    notification_id: id("9999", 3),
    kind: "message_received",
    body: "生徒から新しいメッセージが届きました",
    created_at: at(-1),
    is_read: false,
  },
  {
    notification_id: id("9999", 2),
    kind: "video_review_requested",
    body: "生徒から動画の添削の依頼が届きました",
    created_at: at(-5),
    is_read: false,
  },
  {
    notification_id: id("9999", 1),
    kind: "booking_confirmed",
    body: "新しい予約が入りました",
    created_at: at(-24 * 2),
    is_read: true,
  },
];

/** 契約のパス（/api を除く）と、決まった応答。パスの {…} は 1 つの区切りに合う。 */
/** `query` があれば、そのクエリの値がすべて一致するときだけ使う（同じパスの `query` のない応答より前に置く）。 */
// ビデオ通話の記録。開始の新しい順。通話中（終了の記録なし）の行と、名前の引けないコーチの行を混ぜる。
// 既定の期間（今日までの 7 日）の応答。高橋 コーチで絞ったときは 0 件を返す（0 件の表示を撮る）。
// 山田 花子で絞ったとき（生徒の利用の様子から開く）は山田 花子の行だけを返す。
const LIVE_SESSION_RECORDS: Schemas["LiveLessonSessionRecord"][] = [
  {
    session_id: id("5555", 11),
    booking_id: id("6666", 11),
    coach_id: COACH_ID,
    coach_display_name: "鈴木 コーチ",
    student_id: STUDENT_ID_2,
    student_display_name: "佐藤 一郎",
    started_at: at(-0.5),
    ended_at: null,
    duration_minutes: null,
  },
  {
    session_id: id("5555", 12),
    booking_id: id("6666", 12),
    coach_id: COACH_ID,
    coach_display_name: "鈴木 コーチ",
    student_id: STUDENT_ID,
    student_display_name: "山田 花子",
    started_at: at(-24 - 2),
    ended_at: at(-24 - 1),
    duration_minutes: 60,
  },
  {
    session_id: id("5555", 13),
    booking_id: id("6666", 13),
    coach_id: id("2222", 3),
    student_id: STUDENT_ID,
    student_display_name: "山田 花子",
    started_at: at(-24 * 4 - 3),
    ended_at: at(-24 * 4 - 3 + 0.75),
    duration_minutes: 45,
  },
];

export type Fixture = {
  method: "GET" | "POST" | "PUT";
  path: string;
  query?: Record<string, string>;
  status?: number;
  body: unknown;
};

export const FIXTURES: Fixture[] = [
  {
    method: "GET",
    path: "/me",
    body: {
      account_id: COACH_ID,
      role: "coach",
      display_name: "鈴木 コーチ",
      bio: "テニス歴 20 年。初心者の方のフォーム作りが得意です。",
    } satisfies Schemas["Account"],
  },
  {
    method: "GET",
    path: "/coaches/me/lesson-stats",
    body: {
      coach_id: COACH_ID,
      current_month: {
        month: "2026-10",
        live_lesson_count: 12,
        async_lesson_count: 5,
        review_count: 8,
        average_rating: 4.63,
      },
      previous_month: {
        month: "2026-09",
        live_lesson_count: 9,
        async_lesson_count: 3,
        review_count: 6,
        average_rating: 4.5,
      },
    } satisfies Schemas["CoachLessonStats"],
  },
  { method: "GET", path: "/coaches/me/slots", body: { slots: SLOTS } satisfies Schemas["CoachSlotList"] },
  { method: "GET", path: "/coaches/me/bookings", body: { bookings: BOOKINGS } satisfies Schemas["CoachBookingList"] },
  {
    // 入室前チェックの「入室する」（coach-prejoin-check-failed）。端末の時計が進んでいて、サーバーはまだ開始時刻の前と断る
    method: "POST",
    path: "/live-lesson-sessions",
    status: 409,
    body: { code: "slot_not_started", message: "予約したスロットの開始時刻より前です" } satisfies Schemas["Error"],
  },
  {
    // 予約枠の「枠を公開」で過去の開始を入れたとき（coach-slots-publish-past）。サーバーは 422 で断る
    method: "POST",
    path: "/slots",
    status: 422,
    body: { code: "validation_failed", message: "starts_at is in the past" } satisfies Schemas["Error"],
  },
  { method: "GET", path: "/coaches/me/videos", body: { videos: VIDEOS } satisfies Schemas["CoachVideoList"] },
  {
    method: "GET",
    path: "/coaches/me/lesson-menus",
    body: { coach_id: COACH_ID, lesson_menus: [MENU_FORE, MENU_SERVE, MENU_VOLLEY] } satisfies Schemas["LessonMenuList"],
  },
  { method: "GET", path: "/message-threads", body: { threads: THREADS } satisfies Schemas["MessageThreadList"] },
  {
    method: "GET",
    path: "/message-threads/{thread_id}/messages",
    body: { thread_id: THREAD_ID, messages: MESSAGES } satisfies Schemas["MessageList"],
  },
  { method: "POST", path: "/message-threads/{thread_id}/read", status: 204, body: null },
  {
    method: "GET",
    path: "/notifications/me",
    body: { notifications: NOTIFICATIONS } satisfies Schemas["NotificationList"],
  },
  {
    method: "POST",
    path: "/notifications/me/read",
    body: {
      notifications: NOTIFICATIONS.map((notification) => ({ ...notification, is_read: true })),
    } satisfies Schemas["NotificationList"],
  },
  {
    method: "GET",
    path: "/coaches/{coach_id}/review-summary",
    body: { coach_id: COACH_ID, review_count: 3, average_rating: 4.67 } satisfies Schemas["CoachReviewSummary"],
  },
  // 本部の「コーチの評価」で期間を選んだとき（その期間に付いた評価として新しい 1 件だけを返す）。
  {
    method: "GET",
    path: "/coaches/{coach_id}/reviews",
    query: { from: "2026-10-26", to: "2026-11-01" },
    body: { coach_id: COACH_ID, reviews: REVIEWS.slice(0, 1) } satisfies Schemas["CoachReviewList"],
  },
  // 本部の「コーチの評価」で点数を選んだとき（4 点以下は 1 件、3 点以下は 0 件）。
  {
    method: "GET",
    path: "/coaches/{coach_id}/reviews",
    query: { max_rating: "4" },
    body: {
      coach_id: COACH_ID,
      reviews: REVIEWS.filter((item) => item.rating <= 4),
    } satisfies Schemas["CoachReviewList"],
  },
  {
    method: "GET",
    path: "/coaches/{coach_id}/reviews",
    query: { max_rating: "3" },
    body: { coach_id: COACH_ID, reviews: [] } satisfies Schemas["CoachReviewList"],
  },
  {
    method: "GET",
    path: "/coaches/{coach_id}/reviews",
    body: { coach_id: COACH_ID, reviews: REVIEWS } satisfies Schemas["CoachReviewList"],
  },
  {
    // 山田 花子で絞ったとき（生徒の利用の様子から開く。hq-low-reviews-by-student）は山田 花子の行だけを返す。
    method: "GET",
    path: "/lesson-reviews/low",
    query: { student_id: STUDENT_ID },
    body: {
      max_rating: 2,
      period_days: 30,
      reviews: LOW_REVIEWS.filter((review) => review.student_id === STUDENT_ID),
    } satisfies Schemas["LowRatedLessonReviewList"],
  },
  {
    // 高橋 コーチで絞ったとき（hq-low-reviews-coach）は高橋 コーチの行だけを返す。
    method: "GET",
    path: "/lesson-reviews/low",
    query: { coach_id: COACH_ID_2 },
    body: {
      max_rating: 2,
      period_days: 30,
      reviews: LOW_REVIEWS.filter((review) => review.coach_id === COACH_ID_2),
    } satisfies Schemas["LowRatedLessonReviewList"],
  },
  {
    // フォアハンドの基礎で絞ったとき（hq-low-reviews-lesson-menu）はそのメニューの行だけを返す（メニューのない行は外れる）。
    method: "GET",
    path: "/lesson-reviews/low",
    query: { lesson_menu_id: MENU_FORE.lesson_menu_id },
    body: {
      max_rating: 2,
      period_days: 30,
      reviews: LOW_REVIEWS.filter((review) => review.lesson_menu?.lesson_menu_id === MENU_FORE.lesson_menu_id),
    } satisfies Schemas["LowRatedLessonReviewList"],
  },
  {
    // ゴルフで絞ったとき（hq-low-reviews-sport-empty）は 0 件。
    method: "GET",
    path: "/lesson-reviews/low",
    query: { sport: "golf" },
    body: { max_rating: 2, period_days: 30, reviews: [] } satisfies Schemas["LowRatedLessonReviewList"],
  },
  {
    method: "GET",
    path: "/lesson-reviews/low",
    body: { max_rating: 2, period_days: 30, reviews: LOW_REVIEWS } satisfies Schemas["LowRatedLessonReviewList"],
  },
  {
    method: "GET",
    path: "/live-lesson-sessions",
    query: { student_id: STUDENT_ID },
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      sessions: LIVE_SESSION_RECORDS.filter((session) => session.student_id === STUDENT_ID),
    } satisfies Schemas["LiveLessonSessionRecordList"],
  },
  {
    // テニスで絞ったとき（hq-live-sessions-by-sport）は、テニスのコーチ（鈴木 コーチ）のセッションだけを返す。
    method: "GET",
    path: "/live-lesson-sessions",
    query: { sport: "tennis" },
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      sessions: LIVE_SESSION_RECORDS.filter((session) => session.coach_id === COACH_ID),
    } satisfies Schemas["LiveLessonSessionRecordList"],
  },
  {
    // レッスンメニュー「フォアハンドの基礎」で絞ったとき（hq-live-sessions-by-lesson-menu）は 0 件を返す（メニューで絞って 0 件の表示を撮る）。
    method: "GET",
    path: "/live-lesson-sessions",
    query: { lesson_menu_id: MENU_FORE.lesson_menu_id },
    body: { from: "2026-10-20", to: "2026-10-26", sessions: [] } satisfies Schemas["LiveLessonSessionRecordList"],
  },
  {
    method: "GET",
    path: "/live-lesson-sessions",
    query: { coach_id: COACH_ID_2 },
    body: { from: "2026-10-20", to: "2026-10-26", sessions: [] } satisfies Schemas["LiveLessonSessionRecordList"],
  },
  {
    method: "GET",
    path: "/live-lesson-sessions",
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      sessions: LIVE_SESSION_RECORDS,
    } satisfies Schemas["LiveLessonSessionRecordList"],
  },
  {
    // 山田 花子で絞ったとき（hq-overdue-live-sessions-by-student）は、その生徒のセッションだけを返す。
    method: "GET",
    path: "/live-lesson-sessions/overdue",
    query: { student_id: STUDENT_ID },
    body: {
      older_than_hours: 3,
      sessions: OVERDUE_SESSIONS.filter((session) => session.student_id === STUDENT_ID),
    } satisfies Schemas["OverdueLiveLessonSessionList"],
  },
  {
    // URL の生徒の ID が UUID でないとき（hq-overdue-live-sessions-invalid-student）は 422。
    method: "GET",
    path: "/live-lesson-sessions/overdue",
    query: { student_id: "not-a-uuid" },
    status: 422,
    body: { code: "validation_failed", message: "student_id must be a UUID" } satisfies Schemas["Error"],
  },
  {
    // テニスで絞ったとき（hq-overdue-live-sessions-by-sport）は、テニスのコーチ（鈴木 コーチ）のセッションだけを返す。
    method: "GET",
    path: "/live-lesson-sessions/overdue",
    query: { sport: "tennis" },
    body: {
      older_than_hours: 3,
      sessions: OVERDUE_SESSIONS.filter((session) => session.coach_id === COACH_ID),
    } satisfies Schemas["OverdueLiveLessonSessionList"],
  },
  {
    // 鈴木 コーチで絞ったとき（hq-overdue-live-sessions-by-coach）は、そのコーチのセッションだけを返す。
    method: "GET",
    path: "/live-lesson-sessions/overdue",
    query: { coach_id: COACH_ID },
    body: {
      older_than_hours: 3,
      sessions: OVERDUE_SESSIONS.filter((session) => session.coach_id === COACH_ID),
    } satisfies Schemas["OverdueLiveLessonSessionList"],
  },
  {
    // 高橋 コーチで絞ったときは 0 件を返す（絞り込んで 0 件の表示を撮る。hq-overdue-live-sessions-by-coach-empty）。
    method: "GET",
    path: "/live-lesson-sessions/overdue",
    query: { coach_id: COACH_ID_2 },
    body: { older_than_hours: 3, sessions: [] } satisfies Schemas["OverdueLiveLessonSessionList"],
  },
  {
    // URL のコーチの ID が UUID でないとき（hq-overdue-live-sessions-invalid-coach）は 422。
    method: "GET",
    path: "/live-lesson-sessions/overdue",
    query: { coach_id: "not-a-uuid" },
    status: 422,
    body: { code: "validation_failed", message: "coach_id must be a UUID" } satisfies Schemas["Error"],
  },
  {
    method: "GET",
    path: "/live-lesson-sessions/overdue",
    body: { older_than_hours: 3, sessions: OVERDUE_SESSIONS } satisfies Schemas["OverdueLiveLessonSessionList"],
  },
  {
    method: "GET",
    path: "/coaches/{coach_id}/acceptance-changes",
    body: {
      coach_id: COACH_ID,
      changes: [
        { acceptance_status: "accepting", reason: null, changed_by: HEADQUARTERS_ID, changed_at: at(-26) },
        {
          acceptance_status: "suspended",
          reason: "連絡がつかないため、確認できるまで止める",
          changed_by: HEADQUARTERS_ID,
          changed_at: at(-72),
        },
      ],
    } satisfies Schemas["CoachAcceptanceChangeList"],
  },
  {
    // コーチのお休みの期間（10/28〜11/3）の予約と枠。確定済みの体験予約・メニューつきの確定待ちと、予約のない枠
    method: "GET",
    path: "/coaches/{coach_id}/leave-preview",
    body: {
      coach: { coach_id: COACH_ID, display_name: "鈴木 コーチ", sports: ["tennis"], acceptance_status: "accepting" },
      from: "2026-10-28",
      to: "2026-11-03",
      bookings: [
        {
          booking_id: id("bbbb", 901),
          slot_id: id("dddd", 901),
          starts_at: at(48 + 7),
          ends_at: at(48 + 7.5),
          status: "confirmed",
          is_trial: true,
          student_id: STUDENT_ID,
          student_display_name: "山田 花子",
        },
        {
          booking_id: id("bbbb", 902),
          slot_id: id("dddd", 902),
          starts_at: at(72 + 8),
          ends_at: at(72 + 9),
          status: "pending",
          is_trial: false,
          lesson_menu_name: "フォアハンドの基礎",
          student_id: STUDENT_ID_2,
          student_display_name: "佐藤 次郎",
        },
      ],
      open_slots: [
        {
          slot_id: id("dddd", 903),
          starts_at: at(96 + 7),
          ends_at: at(96 + 7.5),
          capacity: 1,
        },
      ],
      slot_count: 3,
      student_count: 2,
      notification_preview: {
        title: "コーチのお休みで予約をキャンセルしました",
        body: "鈴木 コーチ コーチのお休みのため、10/28（水）19:00〜19:30 の予約を本部でキャンセルしました。今月のビデオ通話の枠は使いません。ほかの枠やコーチを探して予約できます。",
      },
    } satisfies Schemas["CoachLeavePreview"],
  },
  {
    method: "POST",
    path: "/coaches/{coach_id}/leaves",
    status: 201,
    body: {
      coach_leave: {
        coach_leave_id: id("eeee", 901),
        coach_id: COACH_ID,
        from: "2026-10-28",
        to: "2026-11-03",
        note: "",
        created_by: HEADQUARTERS_ID,
        created_at: at(0),
      },
      cancelled_booking_ids: [id("bbbb", 901), id("bbbb", 902)],
      cancelled_booking_count: 2,
      notified_student_count: 2,
      withdrawn_slot_count: 3,
    } satisfies Schemas["CoachLeaveResult"],
  },
  {
    method: "GET",
    path: "/coaches/{coach_id}/overview",
    body: {
      coach_id: COACH_ID,
      display_name: "鈴木 コーチ",
      acceptance_status: "accepting",
      review_count: 3,
      average_rating: 4.67,
      // 低い評価の一覧（LOW_REVIEWS）の鈴木 コーチの 2 件。どちらも対応済みの印はない。
      low_rated_review_count: 2,
      unhandled_low_rated_review_count: 2,
      current_month: {
        month: "2026-10",
        live_lesson_count: 12,
        async_lesson_count: 5,
        review_count: 8,
        average_rating: 4.63,
      },
      previous_month: {
        month: "2026-09",
        live_lesson_count: 9,
        async_lesson_count: 3,
        review_count: 6,
        average_rating: 4.5,
      },
      published_lesson_menu_count: 3,
      returned_video_review_count: 7,
      median_video_review_lead_time_hours: 18.5,
      pending_video_review_count: 2,
      overdue_pending_video_review_count: 1,
      recent_cancelled_booking_count: 2,
      // コーチで絞らない返事のないメッセージの応答（鈴木 コーチの 2 件）と揃える（hq-unanswered-messages-by-coach）。
      unanswered_message_thread_count: 2,
    } satisfies Schemas["CoachOverview"],
  },
  {
    // 受付中で絞ったときは、受付を止めている高橋 コーチを除いた伊藤 コーチだけを返す（hq-incomplete-coach-profiles-accepting）。
    method: "GET",
    path: "/coaches/incomplete-profiles",
    query: { acceptance_status: "accepting" },
    body: {
      coaches: [
        {
          coach_id: id("2222", 3),
          display_name: "伊藤 コーチ",
          missing_fields: ["bio"],
          updated_at: at(-24 * 3),
          registered_at: at(-24 * 30),
        },
      ],
    } satisfies Schemas["IncompleteCoachProfileList"],
  },
  {
    // 表示名で「高橋」を探したとき（hq-incomplete-coach-profiles-search）。
    method: "GET",
    path: "/coaches/incomplete-profiles",
    query: { q: "高橋" },
    body: {
      coaches: [
        {
          coach_id: COACH_ID_2,
          display_name: "高橋 コーチ",
          missing_fields: ["bio"],
          updated_at: null,
          registered_at: at(-24 * 60),
        },
      ],
    } satisfies Schemas["IncompleteCoachProfileList"],
  },
  {
    // 表示名で「山本」を探したときは 0 件を返す（検索語を含めた空の表示を撮る。hq-incomplete-coach-profiles-search-empty）。
    method: "GET",
    path: "/coaches/incomplete-profiles",
    query: { q: "山本" },
    body: { coaches: [] } satisfies Schemas["IncompleteCoachProfileList"],
  },
  {
    // 水泳で絞ったときは 0 件を返す（絞り込んで 0 件の表示を撮る）。
    method: "GET",
    path: "/coaches/incomplete-profiles",
    query: { sport: "swimming" },
    body: { coaches: [] } satisfies Schemas["IncompleteCoachProfileList"],
  },
  {
    method: "GET",
    path: "/coaches/incomplete-profiles",
    body: {
      // 登録の古い順。鈴木 コーチは /me で自己紹介を書いているので載せない。更新の時刻は今は常に null だが、
      // 記録するようになったときの表示も撮るため、記録のある行を 1 つ混ぜる。
      coaches: [
        {
          coach_id: COACH_ID_2,
          display_name: "高橋 コーチ",
          missing_fields: ["bio"],
          updated_at: null,
          registered_at: at(-24 * 60),
        },
        {
          coach_id: id("2222", 3),
          display_name: "伊藤 コーチ",
          missing_fields: ["bio"],
          updated_at: at(-24 * 3),
          registered_at: at(-24 * 30),
        },
      ],
    } satisfies Schemas["IncompleteCoachProfileList"],
  },
  {
    // 表示名で「鈴木」を探したとき（hq-coaches-search）。
    method: "GET",
    path: "/coaches",
    query: { q: "鈴木" },
    body: {
      coaches: [
        { coach_id: COACH_ID, display_name: "鈴木 コーチ", acceptance_status: "accepting", recent_lesson_count: 12 },
      ],
    } satisfies Schemas["CoachList"],
  },
  {
    method: "GET",
    path: "/coaches",
    body: {
      coaches: [
        { coach_id: COACH_ID, display_name: "鈴木 コーチ", acceptance_status: "accepting", recent_lesson_count: 12 },
        { coach_id: COACH_ID_2, display_name: "高橋 コーチ", acceptance_status: "suspended", recent_lesson_count: 0 },
      ],
    } satisfies Schemas["CoachList"],
  },
  // 期間とレッスンメニュー（フォアハンドの基礎）を指定したとき（hq-slot-supply-lesson-menu）。クエリは部分一致で引くので、期間だけの応答より先に置く。
  // 鈴木 コーチがそのメニューに結びつけて公開した枠と、それらの枠への予約だけを数える（テニスのメニューなので、テニスにだけ数が出る）。
  {
    method: "GET",
    path: "/slot-supply",
    query: { from: "2026-11-02", to: "2026-11-08", lesson_menu_id: MENU_FORE.lesson_menu_id },
    body: {
      window_start: "2026-11-01T15:00:00Z",
      window_end: "2026-11-08T15:00:00Z",
      from: "2026-11-02",
      to: "2026-11-08",
      sports: [
        { sport: "tennis", status: "insufficient", published_slots: 2, booking_students: 3 },
        { sport: "golf", status: "not_applicable", published_slots: 0, booking_students: 0 },
        { sport: "baseball", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "soccer", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "basketball", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "swimming", status: "no_activity", published_slots: 0, booking_students: 0 },
      ],
    } satisfies Schemas["SlotSupplyReport"],
  },
  // 期間とスポーツ（ゴルフ）を指定したとき（hq-slot-supply-sport）。クエリは部分一致で引くので、期間だけの応答より先に置く。
  // ゴルフを提供するコーチの枠と予約だけを数える。ほかのスポーツも教えるコーチの枠があれば、そのスポーツにも数が出る。
  {
    method: "GET",
    path: "/slot-supply",
    query: { from: "2026-11-02", to: "2026-11-08", sport: "golf" },
    body: {
      window_start: "2026-11-01T15:00:00Z",
      window_end: "2026-11-08T15:00:00Z",
      from: "2026-11-02",
      to: "2026-11-08",
      sports: [
        { sport: "tennis", status: "sufficient", published_slots: 2, booking_students: 1 },
        { sport: "golf", status: "not_applicable", published_slots: 2, booking_students: 5 },
        { sport: "baseball", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "soccer", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "basketball", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "swimming", status: "no_activity", published_slots: 0, booking_students: 0 },
      ],
    } satisfies Schemas["SlotSupplyReport"],
  },
  // 期間とコーチ（鈴木 コーチ）を指定したとき（hq-slot-supply-coach）。クエリは部分一致で引くので、期間だけの応答より先に置く。
  {
    method: "GET",
    path: "/slot-supply",
    query: { from: "2026-11-02", to: "2026-11-08", coach_id: COACH_ID },
    body: {
      window_start: "2026-11-01T15:00:00Z",
      window_end: "2026-11-08T15:00:00Z",
      from: "2026-11-02",
      to: "2026-11-08",
      sports: [
        { sport: "tennis", status: "insufficient", published_slots: 3, booking_students: 4 },
        { sport: "golf", status: "not_applicable", published_slots: 0, booking_students: 0 },
        { sport: "baseball", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "soccer", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "basketball", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "swimming", status: "no_activity", published_slots: 0, booking_students: 0 },
      ],
    } satisfies Schemas["SlotSupplyReport"],
  },
  // 期間を指定したとき（来週。hq-slot-supply-period）
  {
    method: "GET",
    path: "/slot-supply",
    query: { from: "2026-11-02", to: "2026-11-08" },
    body: {
      window_start: "2026-11-01T15:00:00Z",
      window_end: "2026-11-08T15:00:00Z",
      from: "2026-11-02",
      to: "2026-11-08",
      sports: [
        { sport: "tennis", status: "insufficient", published_slots: 9, booking_students: 7 },
        { sport: "golf", status: "not_applicable", published_slots: 2, booking_students: 5 },
        { sport: "baseball", status: "sufficient", published_slots: 4, booking_students: 1 },
        { sport: "soccer", status: "sufficient", published_slots: 3, booking_students: 0 },
        { sport: "basketball", status: "no_activity", published_slots: 0, booking_students: 0 },
        { sport: "swimming", status: "sufficient", published_slots: 5, booking_students: 2 },
      ],
    } satisfies Schemas["SlotSupplyReport"],
  },
  // 期間を指定して、どのスポーツにも枠も予約もないとき（hq-slot-supply-empty）
  {
    method: "GET",
    path: "/slot-supply",
    query: { from: "2026-12-21", to: "2026-12-27" },
    body: {
      window_start: "2026-12-20T15:00:00Z",
      window_end: "2026-12-27T15:00:00Z",
      from: "2026-12-21",
      to: "2026-12-27",
      sports: (["tennis", "golf", "baseball", "soccer", "basketball", "swimming"] as const).map((sport) => ({
        sport,
        status: sport === "golf" ? ("not_applicable" as const) : ("no_activity" as const),
        published_slots: 0,
        booking_students: 0,
      })),
    } satisfies Schemas["SlotSupplyReport"],
  },
  {
    method: "GET",
    path: "/slot-supply",
    body: {
      window_start: at(-24 * 7),
      window_end: NOW.toISOString(),
      sports: [
        { sport: "tennis", status: "sufficient", published_slots: 42, booking_students: 30 },
        { sport: "golf", status: "not_applicable", published_slots: 8, booking_students: 15 },
        { sport: "baseball", status: "sufficient", published_slots: 12, booking_students: 9 },
        { sport: "soccer", status: "insufficient", published_slots: 4, booking_students: 11 },
        { sport: "basketball", status: "sufficient", published_slots: 6, booking_students: 2 },
        { sport: "swimming", status: "sufficient", published_slots: 10, booking_students: 7 },
      ],
    } satisfies Schemas["SlotSupplyReport"],
  },
  {
    method: "GET",
    path: "/lesson-menus",
    query: { q: "スイング" },
    body: {
      lesson_menus: [
        {
          ...MENU_FORE,
          lesson_menu_id: id("aaaa", 9),
          coach_id: COACH_ID_2,
          sport: "golf",
          name: "スイングの見直し",
          duration_minutes: 90,
          coach_display_name: "高橋 コーチ",
          recent_booking_count: 7,
        },
      ],
    } satisfies Schemas["CoachLessonMenuList"],
  },
  {
    method: "GET",
    path: "/lesson-menus",
    // レッスンの種類で非同期だけ（下の一覧のうち「サーブの添削」）。
    query: { lesson_type: "async_lesson" },
    body: {
      lesson_menus: [{ ...MENU_SERVE, coach_display_name: "鈴木 コーチ", recent_booking_count: 3 }],
    } satisfies Schemas["CoachLessonMenuList"],
  },
  {
    method: "GET",
    path: "/lesson-menus",
    // 直近 30 日に予約のないメニューだけ（下の一覧のうち予約が 0 件の「パターの距離感」）。
    query: { unbooked: "true" },
    body: {
      lesson_menus: [
        {
          ...MENU_FORE,
          lesson_menu_id: id("aaaa", 10),
          coach_id: COACH_ID_2,
          sport: "golf",
          name: "パターの距離感",
          duration_minutes: 30,
          is_published: false,
          coach_display_name: "高橋 コーチ",
          headquarters_unpublish: { unpublished_at: at(-3), reason: "料金の表記が誤っている" },
          recent_booking_count: 0,
        },
      ],
    } satisfies Schemas["CoachLessonMenuList"],
  },
  {
    method: "GET",
    path: "/lesson-menus",
    body: {
      lesson_menus: [
        { ...MENU_FORE, coach_display_name: "鈴木 コーチ", recent_booking_count: 12 },
        { ...MENU_SERVE, coach_display_name: "鈴木 コーチ", recent_booking_count: 3 },
        {
          ...MENU_FORE,
          lesson_menu_id: id("aaaa", 9),
          coach_id: COACH_ID_2,
          sport: "golf",
          name: "スイングの見直し",
          duration_minutes: 90,
          coach_display_name: "高橋 コーチ",
          recent_booking_count: 7,
        },
        {
          ...MENU_FORE,
          lesson_menu_id: id("aaaa", 10),
          coach_id: COACH_ID_2,
          sport: "golf",
          name: "パターの距離感",
          duration_minutes: 30,
          is_published: false,
          coach_display_name: "高橋 コーチ",
          headquarters_unpublish: { unpublished_at: at(-3), reason: "料金の表記が誤っている" },
          recent_booking_count: 0,
        },
      ],
    } satisfies Schemas["CoachLessonMenuList"],
  },
  {
    method: "GET",
    path: "/lesson-menus/{lesson_menu_id}/publication-changes",
    // 一覧の「パターの距離感」（本部が非公開にしている）の記録。一度公開に戻したあと、また非公開にした。
    body: {
      lesson_menu_id: id("aaaa", 10),
      changes: [
        { action: "unpublished", reason: "料金の表記が誤っている", changed_by: HEADQUARTERS_ID, changed_at: at(-3) },
        { action: "republished", reason: null, changed_by: HEADQUARTERS_ID, changed_at: at(-24 * 2) },
        { action: "unpublished", reason: "", changed_by: HEADQUARTERS_ID, changed_at: at(-24 * 4) },
      ],
    } satisfies Schemas["LessonMenuPublicationChangeList"],
  },
  {
    // 鈴木 コーチで絞ったとき（hq-students-by-coach）。そのコーチを予約したことのある生徒だけ。
    method: "GET",
    path: "/students",
    query: { coach_id: COACH_ID },
    body: {
      students: [
        {
          student_id: STUDENT_ID,
          display_name: YAMADA.name,
          sport: "tennis",
          contract_type: "subscription",
          plan: "standard",
          last_booked_at: at(-24),
          recent_booking_count: 4,
          registered_at: at(-24 * 5),
        },
      ],
    } satisfies Schemas["StudentList"],
  },
  {
    // 高橋 コーチで絞ったときは 0 件を返す（絞り込んで 0 件の表示を撮る。hq-students-by-coach-empty）。
    method: "GET",
    path: "/students",
    query: { coach_id: COACH_ID_2 },
    body: { students: [] } satisfies Schemas["StudentList"],
  },
  {
    // 返事を待っているメッセージのある生徒だけに絞ったとき（hq-students-has-unanswered）。山田 花子だけ。
    method: "GET",
    path: "/students",
    query: { has_unanswered: "true" },
    body: {
      students: [
        {
          student_id: STUDENT_ID,
          display_name: YAMADA.name,
          sport: "tennis",
          contract_type: "subscription",
          plan: "standard",
          last_booked_at: at(-24),
          recent_booking_count: 4,
          registered_at: at(-24 * 5),
        },
      ],
    } satisfies Schemas["StudentList"],
  },
  {
    // URL のコーチの ID が UUID でないとき（hq-students-invalid-coach）は 422。
    method: "GET",
    path: "/students",
    query: { coach_id: "not-a-uuid" },
    status: 422,
    body: { code: "validation_failed", message: "coach_id must be a UUID" } satisfies Schemas["Error"],
  },
  {
    method: "GET",
    path: "/students",
    body: {
      students: [
        {
          student_id: STUDENT_ID,
          display_name: YAMADA.name,
          sport: "tennis",
          contract_type: "subscription",
          plan: "standard",
          last_booked_at: at(-24),
          recent_booking_count: 4,
          registered_at: at(-24 * 5),
        },
        {
          student_id: STUDENT_ID_2,
          display_name: SATO.name,
          sport: "golf",
          contract_type: "one_shot",
          plan: null,
          last_booked_at: at(-24 * 3),
          recent_booking_count: 1,
          registered_at: at(-24 * 12),
        },
        {
          student_id: "11111111-1111-4111-8111-111111111113",
          display_name: "田中 美咲",
          sport: "swimming",
          contract_type: null,
          plan: null,
          last_booked_at: null,
          recent_booking_count: 0,
          registered_at: at(-24 * 20),
        },
      ],
    } satisfies Schemas["StudentList"],
  },
  {
    method: "GET",
    path: "/students/{student_id}/activity",
    body: {
      student_id: STUDENT_ID,
      display_name: YAMADA.name,
      upcoming_bookings: HQ_BOOKINGS.filter(
        (item) => item.booking.student_id === STUDENT_ID && item.booking.status !== "cancelled",
      ).map((item) => ({ booking: item.booking, coach_display_name: item.coach_display_name })),
      recent_lessons: [
        {
          lesson_id: id("5555", 1),
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          lesson_type: "live_lesson",
          completed_at: at(-24 * 2),
        },
        {
          lesson_id: id("5555", 2),
          coach_id: COACH_ID_2,
          coach_display_name: TAKAHASHI_COACH.name,
          lesson_type: "async_lesson",
          completed_at: at(-24 * 6),
        },
      ],
      cancelled_booking_count: 1,
      submitted_video_count: 2,
      pending_video_review_count: 1,
      // 生徒で絞った返事のないメッセージの応答（1 件）と揃える。0 件だと一覧への入口が出ず hq-unanswered-messages-by-student が撮れない
      unanswered_message_thread_count: 1,
      // 生徒で絞った低い評価の応答（山田 花子の 1 件。対応済みの印はない）と揃える。0 件だと一覧への入口が出ず
      // hq-low-reviews-by-student が撮れない
      low_rated_review_count: 1,
      unhandled_low_rated_review_count: 1,
    } satisfies Schemas["StudentActivity"],
  },
  {
    // 未読だけに絞った生徒の受け取ったお知らせ（hq-student-announcements-unread）。条件なしの応答のうち未読の 2 件を返す。
    // mockApi は先に一致した応答を返すので、絞らない応答より前に置く。
    method: "GET",
    path: "/students/{student_id}/announcements",
    query: { unread_only: "true" },
    body: {
      announcements: [
        {
          announcement_id: id("8888", 3),
          title: "年末年始の営業について",
          sent_at: at(-12),
          is_read: false,
          is_withdrawn: false,
        },
        {
          announcement_id: id("8888", 4),
          title: "テニスの新しいコーチのご紹介",
          sent_at: at(-24 * 5),
          is_read: false,
          is_withdrawn: true,
        },
      ],
    } satisfies Schemas["StudentAnnouncementList"],
  },
  {
    method: "GET",
    path: "/students/{student_id}/announcements",
    body: {
      announcements: [
        {
          announcement_id: id("8888", 3),
          title: "年末年始の営業について",
          sent_at: at(-12),
          is_read: false,
          is_withdrawn: false,
        },
        {
          announcement_id: id("8888", 1),
          title: "メンテナンスのお知らせ",
          sent_at: at(-24 * 4),
          is_read: true,
          is_withdrawn: false,
        },
        {
          announcement_id: id("8888", 4),
          title: "テニスの新しいコーチのご紹介",
          sent_at: at(-24 * 5),
          is_read: false,
          is_withdrawn: true,
        },
      ],
    } satisfies Schemas["StudentAnnouncementList"],
  },
  {
    // 生徒で絞った予約の一覧。mockApi は先に一致した応答を返すので、絞らない `/bookings` より前に置く。
    method: "GET",
    path: "/bookings",
    query: { student_id: STUDENT_ID },
    body: {
      from: "2026-10-26",
      to: "2026-11-01",
      bookings: HQ_BOOKINGS.filter((item) => item.booking.student_id === STUDENT_ID),
    } satisfies Schemas["HeadquartersBookingList"],
  },
  {
    // レッスンメニュー「フォアハンドの基礎」で絞った予約の一覧（hq-bookings-by-lesson-menu）。絞らない `/bookings` より前に置く。
    method: "GET",
    path: "/bookings",
    query: { lesson_menu_id: MENU_FORE.lesson_menu_id },
    body: {
      from: "2026-10-26",
      to: "2026-11-01",
      bookings: HQ_BOOKINGS.filter(
        (item) => item.booking.lesson_menu?.lesson_menu_id === MENU_FORE.lesson_menu_id,
      ),
    } satisfies Schemas["HeadquartersBookingList"],
  },
  {
    // テニスで絞った予約の一覧（hq-bookings-by-sport）。テニスのコーチ（鈴木 コーチ）の枠への予約だけを返す。絞らない `/bookings` より前に置く。
    method: "GET",
    path: "/bookings",
    query: { sport: "tennis" },
    body: {
      from: "2026-10-26",
      to: "2026-11-01",
      bookings: HQ_BOOKINGS.filter((item) => item.booking.coach_id === COACH_ID),
    } satisfies Schemas["HeadquartersBookingList"],
  },
  {
    method: "GET",
    path: "/bookings",
    body: {
      from: "2026-10-26",
      to: "2026-11-01",
      bookings: HQ_BOOKINGS,
    } satisfies Schemas["HeadquartersBookingList"],
  },
  {
    // レッスンメニュー「フォアハンドの基礎」で絞ったとき（hq-cancelled-bookings-lesson-menu）。そのメニューのキャンセルだけを返す。
    method: "GET",
    path: "/bookings/cancelled",
    query: { lesson_menu_id: MENU_FORE.lesson_menu_id },
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      bookings: [
        {
          booking_id: id("4444", 21),
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          starts_at: at(24 * 2),
          cancelled_at: at(-3),
          cancelled_by: "student",
        },
      ],
    } satisfies Schemas["CancelledBookingList"],
  },
  {
    // 本部で絞ったとき（hq-cancelled-bookings-by-canceller）。鈴木 コーチのお休みで本部がまとめてキャンセルした 2 件だけを返す。
    method: "GET",
    path: "/bookings/cancelled",
    query: { cancelled_by: "headquarters" },
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      bookings: [
        {
          booking_id: id("4444", 24),
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          starts_at: at(24 * 3),
          cancelled_at: at(-24),
          cancelled_by: "headquarters",
        },
        {
          booking_id: id("4444", 25),
          student_id: SATO.id,
          student_display_name: SATO.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          starts_at: at(24 * 4),
          cancelled_at: at(-24),
          cancelled_by: "headquarters",
        },
      ],
    } satisfies Schemas["CancelledBookingList"],
  },
  {
    // コーチで絞ったとき（hq-cancelled-bookings-by-canceller-empty）。今コーチは予約をキャンセルできないので 0 件。
    method: "GET",
    path: "/bookings/cancelled",
    query: { cancelled_by: "coach" },
    body: { from: "2026-10-20", to: "2026-10-26", bookings: [] } satisfies Schemas["CancelledBookingList"],
  },
  {
    // テニスで絞ったとき（hq-cancelled-bookings-by-sport）。鈴木 コーチの枠のキャンセルだけを返す。
    method: "GET",
    path: "/bookings/cancelled",
    query: { sport: "tennis" },
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      bookings: [
        {
          booking_id: id("4444", 21),
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          starts_at: at(24 * 2),
          cancelled_at: at(-3),
          cancelled_by: "student",
        },
        {
          booking_id: id("4444", 23),
          student_id: "11111111-1111-4111-8111-111111111113",
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          starts_at: at(-24 * 4),
          cancelled_at: at(-24 * 5 - 1),
          cancelled_by: null,
        },
      ],
    } satisfies Schemas["CancelledBookingList"],
  },
  {
    // コーチで絞ったとき（hq-cancelled-bookings-coach）。高橋 コーチの枠のキャンセルだけを返す。
    method: "GET",
    path: "/bookings/cancelled",
    query: { coach_id: COACH_ID_2 },
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      bookings: [
        {
          booking_id: id("4444", 22),
          student_id: SATO.id,
          student_display_name: SATO.name,
          coach_id: COACH_ID_2,
          coach_display_name: TAKAHASHI_COACH.name,
          starts_at: at(-24 * 1 + 6),
          cancelled_at: at(-24 * 2 - 4.5),
          cancelled_by: "student",
        },
      ],
    } satisfies Schemas["CancelledBookingList"],
  },
  {
    // mockApi は先に一致した応答を返すので、`/bookings/{booking_id}` より前に置く（でないと cancelled を ID として拾う）。
    method: "GET",
    path: "/bookings/cancelled",
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      bookings: [
        {
          booking_id: id("4444", 21),
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          starts_at: at(24 * 2),
          cancelled_at: at(-3),
          cancelled_by: "student",
        },
        {
          booking_id: id("4444", 22),
          student_id: SATO.id,
          student_display_name: SATO.name,
          coach_id: COACH_ID_2,
          coach_display_name: TAKAHASHI_COACH.name,
          starts_at: at(-24 * 1 + 6),
          cancelled_at: at(-24 * 2 - 4.5),
          cancelled_by: "student",
        },
        {
          booking_id: id("4444", 23),
          student_id: "11111111-1111-4111-8111-111111111113",
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          starts_at: at(-24 * 4),
          cancelled_at: at(-24 * 5 - 1),
          cancelled_by: null,
        },
      ],
    } satisfies Schemas["CancelledBookingList"],
  },
  // 予約の詳細の状態が変わった記録の状態ごと（hq-booking-detail-*。specs/ux/issue-1030.yaml）。ID ごとの応答なので、
  // `/bookings/{booking_id}` より前に置く。
  ...HQ_BOOKING_DETAIL_STATES.map(({ bookingId, startsAt = at(48), statusChanges }) => ({
    method: "GET" as const,
    path: `/bookings/${bookingId}`,
    body: {
      booking: {
        ...HQ_BOOKINGS[3]!.booking,
        booking_id: bookingId,
        status: statusChanges.length === 0 ? ("confirmed" as const) : ("cancelled" as const),
        starts_at: startsAt,
        confirmed_at: at(-24 * 2),
        cancelled_at: statusChanges.length === 0 ? null : at(-2),
      },
      slot: {
        ...BOOKINGS[1]!.slot,
        starts_at: startsAt,
        ends_at: new Date(Date.parse(startsAt) + 3_600_000).toISOString(),
        status: "open",
      },
      status_changes: [{ status: "confirmed", changed_at: at(-24 * 2) }, ...statusChanges],
      coach_display_name: HQ_BOOKINGS[3]!.coach_display_name,
      student_display_name: HQ_BOOKINGS[3]!.student_display_name,
    } satisfies Schemas["HeadquartersBookingDetail"],
  })),
  {
    // 予約の詳細は、一覧のキャンセル済みの予約（確定待ちのままキャンセル）を返す。
    method: "GET",
    path: "/bookings/{booking_id}",
    body: {
      booking: HQ_BOOKINGS[3]!.booking,
      slot: { ...BOOKINGS[1]!.slot, starts_at: HQ_BOOKINGS[3]!.booking.starts_at, ends_at: at(49), status: "open" },
      status_changes: [{ status: "cancelled", changed_at: at(-2) }],
      coach_display_name: HQ_BOOKINGS[3]!.coach_display_name,
      student_display_name: HQ_BOOKINGS[3]!.student_display_name,
    } satisfies Schemas["HeadquartersBookingDetail"],
  },
  {
    // 下書きを題で探して見つからないとき（hq-announcement-drafts-search）。0 件の表示（語を含めた文言）を撮る。
    method: "GET",
    path: "/announcement-drafts",
    query: { q: "夏休み" },
    body: { announcement_drafts: [] } satisfies Schemas["AnnouncementDraftList"],
  },
  {
    // 下書きを最後に直した期間とスポーツで絞って見つからないとき（hq-announcement-drafts-filter）。0 件の表示（期間と競技を含めた文言）を撮る。
    method: "GET",
    path: "/announcement-drafts",
    query: { from: "2026-09-01", to: "2026-09-30", sport: "golf" },
    body: { announcement_drafts: [] } satisfies Schemas["AnnouncementDraftList"],
  },
  {
    // 送ったお知らせを題で探して見つからないとき（hq-announcements-search）。0 件の表示（語を含めた文言）を撮る。
    // 期間は既定（今日までの 7 日）のまま。既定の期間の `/announcements` より前に置く。
    method: "GET",
    path: "/announcements",
    query: { q: "夏休み" },
    body: { announcements: [] } satisfies Schemas["AnnouncementList"],
  },
  {
    // 送ったお知らせをスポーツで絞って見つからないとき（hq-announcements-sport）。0 件の表示（競技を含めた文言）を撮る。
    // 期間は既定（今日までの 7 日）のまま。既定の期間の `/announcements` より前に置く。
    method: "GET",
    path: "/announcements",
    query: { sport: "golf" },
    body: { announcements: [] } satisfies Schemas["AnnouncementList"],
  },
  {
    // 送ったお知らせを取り下げたかで絞ったとき（hq-announcements-withdrawn）。取り下げたもの（既定の一覧の 2 件目）だけを返す。
    // 期間は既定（今日までの 7 日）のまま。既定の期間の `/announcements` より前に置く。
    method: "GET",
    path: "/announcements",
    query: { withdrawn: "true" },
    body: {
      announcements: [
        {
          announcement_id: id("8888", 5),
          sender_id: "99999999-9999-4999-8999-999999999999",
          title: "テニスのコートの案内（誤り）",
          body: "11月からコートの予約はアプリの「予約」から行えます。",
          sport: "tennis",
          recipient_count: 15,
          read_count: 3,
          sent_at: at(-24 * 2),
          withdrawal: { withdrawn_by: "99999999-9999-4999-8999-999999999999", withdrawn_at: at(-24 * 2 + 1) },
        },
      ],
    } satisfies Schemas["AnnouncementList"],
  },
  {
    // 期間で絞ったとき（hq-announcements-period）。この期間には送っていない（0 件の表示を撮る）。
    // mockApi は先に一致した応答を返すので、既定の期間の `/announcements` より前に置く。
    method: "GET",
    path: "/announcements",
    query: { from: "2026-09-01", to: "2026-09-30" },
    body: { announcements: [] } satisfies Schemas["AnnouncementList"],
  },
  {
    method: "GET",
    path: "/announcements",
    body: {
      announcements: [
        {
          announcement_id: id("8888", 2),
          sender_id: "99999999-9999-4999-8999-999999999999",
          title: "水泳の体験予約キャンペーン",
          body: "今月中に体験予約すると、2 回目のレッスンが無料になります。\nこの機会にぜひお試しください。",
          sport: "swimming",
          recipient_count: 8,
          read_count: 5,
          sent_at: at(-24),
          withdrawal: null,
        },
        {
          announcement_id: id("8888", 5),
          sender_id: "99999999-9999-4999-8999-999999999999",
          title: "テニスのコートの案内（誤り）",
          body: "11月からコートの予約はアプリの「予約」から行えます。",
          sport: "tennis",
          recipient_count: 15,
          read_count: 3,
          sent_at: at(-24 * 2),
          withdrawal: { withdrawn_by: "99999999-9999-4999-8999-999999999999", withdrawn_at: at(-24 * 2 + 1) },
        },
        {
          announcement_id: id("8888", 1),
          sender_id: "99999999-9999-4999-8999-999999999999",
          title: "メンテナンスのお知らせ",
          body: "10月30日（木）2:00〜4:00 はメンテナンスのため、アプリを使えません。",
          sport: null,
          recipient_count: 120,
          read_count: 87,
          sent_at: at(-24 * 4),
          withdrawal: null,
        },
      ],
    } satisfies Schemas["AnnouncementList"],
  },
  // 高橋 コーチで絞った返事のないメッセージ（hq-unanswered-messages-coach-empty）は 0 件。条件なしの応答より前に置く。
  {
    method: "GET",
    path: "/message-threads/unanswered",
    query: { coach_id: COACH_ID_2 },
    body: { older_than_hours: 24, threads: [] } satisfies Schemas["UnansweredMessageThreadList"],
  },
  // 生徒で絞った返事のないメッセージ（山田 花子）。条件なしの応答より前に置く（先に合ったものを返す）。
  {
    method: "GET",
    path: "/message-threads/unanswered",
    query: { student_id: YAMADA.id },
    body: {
      older_than_hours: 24,
      threads: [
        {
          thread_id: THREAD_ID,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          waiting_since: at(-30.5),
          elapsed_sec: 30.5 * 3600,
        },
      ],
    } satisfies Schemas["UnansweredMessageThreadList"],
  },
  {
    method: "GET",
    path: "/message-threads/unanswered",
    body: {
      older_than_hours: 24,
      threads: [
        {
          thread_id: id("cccc", 1),
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          student_id: SATO.id,
          student_display_name: SATO.name,
          waiting_since: at(-75.25),
          elapsed_sec: 75.25 * 3600,
        },
        {
          thread_id: THREAD_ID,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          waiting_since: at(-30.5),
          elapsed_sec: 30.5 * 3600,
        },
      ],
    } satisfies Schemas["UnansweredMessageThreadList"],
  },
  // コーチで絞った返却待ち（高橋 コーチ）。条件なしの応答より前に置く（先に合ったものを返す）。
  {
    method: "GET",
    path: "/video-reviews/pending",
    query: { coach_id: COACH_ID_2 },
    body: {
      as_of: NOW.toISOString(),
      videos: [
        {
          video_id: id("5555", 21),
          sport: "golf",
          student_id: SATO.id,
          student_display_name: SATO.name,
          coach_id: COACH_ID_2,
          coach_display_name: TAKAHASHI_COACH.name,
          submitted_at: at(-101.5),
          elapsed_hours: 101,
        },
      ],
    } satisfies Schemas["PendingVideoReviewList"],
  },
  // レッスンメニューで絞った返却待ち（サーブの添削）。返却待ちがないメニューで、選んだメニューを含めた空の表示を撮る。
  // 条件なしの応答より前に置く（先に合ったものを返す）。
  {
    method: "GET",
    path: "/video-reviews/pending",
    query: { lesson_menu_id: MENU_SERVE.lesson_menu_id },
    body: { as_of: NOW.toISOString(), videos: [] } satisfies Schemas["PendingVideoReviewList"],
  },
  // 生徒で絞った返却待ち（山田 花子）。条件なしの応答より前に置く（先に合ったものを返す）。
  {
    method: "GET",
    path: "/video-reviews/pending",
    query: { student_id: YAMADA.id },
    body: {
      as_of: NOW.toISOString(),
      videos: [
        {
          video_id: id("5555", 22),
          sport: "tennis",
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          submitted_at: at(-52.25),
          elapsed_hours: 52,
        },
      ],
    } satisfies Schemas["PendingVideoReviewList"],
  },
  {
    method: "GET",
    path: "/video-reviews/pending",
    body: {
      as_of: NOW.toISOString(),
      videos: [
        {
          video_id: id("5555", 21),
          sport: "golf",
          student_id: SATO.id,
          student_display_name: SATO.name,
          coach_id: COACH_ID_2,
          coach_display_name: TAKAHASHI_COACH.name,
          submitted_at: at(-101.5),
          elapsed_hours: 101,
        },
        {
          video_id: id("5555", 22),
          sport: "tennis",
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          submitted_at: at(-52.25),
          elapsed_hours: 52,
        },
        {
          video_id: id("5555", 23),
          sport: "tennis",
          student_id: "11111111-1111-4111-8111-111111111113",
          student_display_name: "田中 美咲",
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          submitted_at: at(-5),
          elapsed_hours: 5,
        },
      ],
    } satisfies Schemas["PendingVideoReviewList"],
  },
  {
    // 生徒の ID が UUID でないとき（hq-returned-video-reviews-invalid-student）は 422。生徒の名前も読めない。
    method: "GET",
    path: "/video-reviews/returned",
    query: { student_id: "not-a-uuid" },
    status: 422,
    body: { code: "validation_failed", message: "student_id must be a UUID" } satisfies Schemas["Error"],
  },
  {
    method: "GET",
    path: "/students/not-a-uuid/activity",
    status: 422,
    body: { code: "validation_failed", message: "student_id must be a UUID" } satisfies Schemas["Error"],
  },
  {
    // レッスンメニューで絞ったとき（hq-returned-video-reviews-lesson-menu。サーブの添削）は 0 件。選んだメニューを含めた空の表示を撮る。
    method: "GET",
    path: "/video-reviews/returned",
    query: { lesson_menu_id: MENU_SERVE.lesson_menu_id },
    body: { from: "2026-10-20", to: "2026-10-26", reviews: [] } satisfies Schemas["ReturnedVideoReviewList"],
  },
  {
    // 山田 花子と高橋 コーチの両方で絞ったとき（hq-returned-video-reviews-by-student-empty）は 0 件。
    method: "GET",
    path: "/video-reviews/returned",
    query: { student_id: YAMADA.id, coach_id: COACH_ID_2 },
    body: { from: "2026-10-20", to: "2026-10-26", reviews: [] } satisfies Schemas["ReturnedVideoReviewList"],
  },
  {
    // 山田 花子で絞ったとき（hq-returned-video-reviews-by-student）は、山田 花子が送った動画の記録だけ。
    method: "GET",
    path: "/video-reviews/returned",
    query: { student_id: YAMADA.id },
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      reviews: [
        {
          video_id: id("5555", 31),
          sport: "tennis",
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          submitted_at: at(-20.5),
          returned_at: at(-2),
          lead_time_hours: 18,
        },
      ],
    } satisfies Schemas["ReturnedVideoReviewList"],
  },
  {
    method: "GET",
    path: "/video-reviews/returned",
    body: {
      from: "2026-10-20",
      to: "2026-10-26",
      reviews: [
        {
          video_id: id("5555", 31),
          sport: "tennis",
          student_id: YAMADA.id,
          student_display_name: YAMADA.name,
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          submitted_at: at(-20.5),
          returned_at: at(-2),
          lead_time_hours: 18,
        },
        {
          video_id: id("5555", 32),
          sport: "golf",
          student_id: SATO.id,
          student_display_name: SATO.name,
          coach_id: COACH_ID_2,
          coach_display_name: TAKAHASHI_COACH.name,
          submitted_at: at(-24 * 5 - 3.25),
          returned_at: at(-24 * 2),
          lead_time_hours: 75,
        },
        {
          video_id: id("5555", 33),
          sport: "tennis",
          student_id: "11111111-1111-4111-8111-111111111113",
          coach_id: COACH_ID,
          coach_display_name: SUZUKI_COACH.name,
          submitted_at: at(-24 * 6 - 6),
          returned_at: at(-24 * 6 + 0.75),
          lead_time_hours: 6,
        },
      ],
    } satisfies Schemas["ReturnedVideoReviewList"],
  },
];
