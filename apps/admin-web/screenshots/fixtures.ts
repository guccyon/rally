import type { Schemas } from "@/api/client";

// 画面のキャプチャ用の決まった応答。型は契約から生成した Schemas だけを使い、契約が変われば typecheck で気づく。
// 時刻は「今」（NOW）を基準に並べる。今は 2026-10-26 12:00（日本時間）に固定する。

export const NOW = new Date("2026-10-26T03:00:00Z");

const COACH_ID = "22222222-2222-4222-8222-222222222222";
const COACH_ID_2 = "22222222-2222-4222-8222-222222222223";
const STUDENT_ID = "11111111-1111-4111-8111-111111111111";
const STUDENT_ID_2 = "11111111-1111-4111-8111-111111111112";
const THREAD_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

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

const BOOKINGS: Schemas["CoachBooking"][] = [
  booking(1, SLOTS[0]!.slot, "confirmed", YAMADA),
  booking(2, SLOTS[1]!.slot, "pending", SATO),
  booking(3, SLOTS[3]!.slot, "confirmed", YAMADA),
];

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
  review(3, 5, "次回もお願いします！", 24 * 12),
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

const LOW_REVIEWS: Schemas["LowRatedLessonReview"][] = [
  lowReview(11, 1, "通話が途中で何度も切れて、ほとんど教えてもらえませんでした。", 6, SUZUKI_COACH, YAMADA),
  lowReview(12, 2, "説明が早口で、何を直せばいいのか分かりにくかったです。", 24 * 2, TAKAHASHI_COACH, SATO, false),
  lowReview(13, 2, "開始が 10 分遅れました。", 24 * 4, SUZUKI_COACH, SATO),
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
export type Fixture = { method: "GET" | "POST" | "PUT"; path: string; status?: number; body: unknown };

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
  {
    method: "GET",
    path: "/coaches/{coach_id}/reviews",
    body: { coach_id: COACH_ID, reviews: REVIEWS } satisfies Schemas["CoachReviewList"],
  },
  {
    method: "GET",
    path: "/lesson-reviews/low",
    body: { max_rating: 2, period_days: 30, reviews: LOW_REVIEWS } satisfies Schemas["LowRatedLessonReviewList"],
  },
  {
    method: "GET",
    path: "/coaches",
    body: {
      coaches: [
        { coach_id: COACH_ID, display_name: "鈴木 コーチ", acceptance_status: "accepting" },
        { coach_id: COACH_ID_2, display_name: "高橋 コーチ", acceptance_status: "accepting" },
      ],
    } satisfies Schemas["CoachList"],
  },
  {
    method: "GET",
    path: "/slot-supply",
    body: {
      window_start: at(-24 * 7),
      window_end: NOW.toISOString(),
      sports: [
        { sport: "tennis", status: "sufficient", published_slots: 42, booking_students: 30 },
        { sport: "golf", status: "insufficient", published_slots: 8, booking_students: 15 },
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
    body: {
      lesson_menus: [
        { ...MENU_FORE, coach_display_name: "鈴木 コーチ" },
        { ...MENU_SERVE, coach_display_name: "鈴木 コーチ" },
        {
          ...MENU_FORE,
          lesson_menu_id: id("aaaa", 9),
          coach_id: COACH_ID_2,
          sport: "golf",
          name: "スイングの見直し",
          duration_minutes: 90,
          coach_display_name: "高橋 コーチ",
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
        },
      ],
    } satisfies Schemas["CoachLessonMenuList"],
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
          registered_at: at(-24 * 5),
        },
        {
          student_id: STUDENT_ID_2,
          display_name: SATO.name,
          sport: "golf",
          contract_type: "one_shot",
          plan: null,
          last_booked_at: at(-24 * 3),
          registered_at: at(-24 * 12),
        },
        {
          student_id: "11111111-1111-4111-8111-111111111113",
          display_name: "田中 美咲",
          sport: "swimming",
          contract_type: null,
          plan: null,
          last_booked_at: null,
          registered_at: at(-24 * 20),
        },
      ],
    } satisfies Schemas["StudentList"],
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
];
