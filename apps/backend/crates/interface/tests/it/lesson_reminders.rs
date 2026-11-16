//! レッスンのリマインドの統合テスト（時刻を進める試験）。定期の処理（`SendLessonReminders`）を時計を進めながら呼び、
//! student-app と admin-web の `GET /notifications/me`（`listMyNotifications`）で、確定した同期レッスンの予約に前日と
//! 開始 1 時間前のリマインドが生徒とコーチに 1 件ずつ届くこと・同じ窓で何度回しても増えないこと・キャンセルした予約と
//! コーチのお休みで消えた予約には届かないこと・`PUT /notification-settings/me`（`updateMyNotificationSettings`）で
//! `lesson_reminder` を切った生徒には届かず、そのコーチには届くことを確かめる。レスポンスの形は、生徒は student-app、
//! コーチは admin-web の OpenAPI のスキーマで検査する。

use std::time::{Duration, SystemTime};

use application::{
    identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier},
    in_memory::{
        FixedCoachLeaveNotices, InMemoryAnnouncementRepository, InMemoryBookingRepository,
        InMemoryNotificationRepository, InMemoryNotificationSettingsRepository,
        InMemorySlotRepository, ManualClock, RecordingPushSender,
    },
    notification::{
        GetNotificationSettings, ListMyNotifications, MarkMyNotificationsRead, SendLessonReminders,
        UpdateNotificationSettings,
    },
    scheduling::{BookingRepository, SchedulingLessonReminderTargetLookup, booking_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{AccountId, Role},
    scheduling::{Booking, BookingId, Slot, SlotId, SlotSeats, SlotStatus},
};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::support;

/// `<役割>:<ID の数値>` の形のトークンを通す偽物（interface は infra のモックに依存できない）。
struct FakeTokens;

impl TokenIssuer for FakeTokens {
    fn issue(&self, _claims: TokenClaims) -> AccessToken {
        unreachable!("このテストでは発行しない")
    }
}

impl TokenVerifier for FakeTokens {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        let (role, id) = token.split_once(':').ok_or(InvalidToken)?;
        let role = match role {
            "student" => Role::Student,
            "coach" => Role::Coach,
            _ => return Err(InvalidToken),
        };
        let id = id.parse().map_err(|_| InvalidToken)?;
        Ok(TokenClaims {
            account_id: AccountId::from_u128(id),
            role,
        })
    }
}

/// 予約を確定したまま受ける生徒
const STUDENT: &str = "student:1";
/// 1 時間前の窓の前にキャンセルする生徒
const CANCELLING_STUDENT: &str = "student:2";
/// リマインドを切る生徒
const OPTED_OUT_STUDENT: &str = "student:3";
/// コーチのお休みで本部に予約をキャンセルされる生徒
const ON_LEAVE_STUDENT: &str = "student:4";
const COACH: &str = "coach:9";

const HOUR: Duration = Duration::from_secs(60 * 60);

/// レッスンの開始（2026-11-20T10:00:00Z）
fn start() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_795_168_800)
}

fn account(token: &str) -> AccountId {
    AccountId::from_u128(token.split_once(':').unwrap().1.parse().unwrap())
}

type Reminders = SendLessonReminders<
    SchedulingLessonReminderTargetLookup<InMemoryBookingRepository>,
    InMemoryNotificationSettingsRepository,
    InMemoryNotificationRepository,
    RecordingPushSender,
    ManualClock,
>;

struct Fixture {
    router: Router,
    bookings: InMemoryBookingRepository,
    clock: ManualClock,
    reminders: Reminders,
    /// 生徒ごとの予約（`[STUDENT, CANCELLING_STUDENT, OPTED_OUT_STUDENT, ON_LEAVE_STUDENT]` の順）
    booked: Vec<Booking>,
}

/// 同じコーチの、同じ時刻に始まる 4 つの枠を、4 人の生徒がそれぞれ予約して確定した状態。
async fn fixture() -> Fixture {
    let coach = account(COACH);
    let slots = InMemorySlotRepository::with_slots((1..=4).map(|id| {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            start(),
            start() + HOUR,
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
    }));
    let bookings = InMemoryBookingRepository::new(slots);
    let booked_at = start() - HOUR * 24 * 7;
    let mut booked = Vec::new();
    for (slot, student) in [
        STUDENT,
        CANCELLING_STUDENT,
        OPTED_OUT_STUDENT,
        ON_LEAVE_STUDENT,
    ]
    .into_iter()
    .enumerate()
    {
        let student = account(student);
        let booking = bookings
            .reserve(
                SlotId::from_u128(slot as u128 + 1),
                student,
                |slot, existing| {
                    Booking::reserve(
                        bookings.next_id(),
                        student,
                        false,
                        slot,
                        existing,
                        booked_at,
                    )
                },
            )
            .await
            .unwrap();
        let booking = bookings
            .update(booking.id(), |booking, slot| {
                booking.confirm(student, slot, booked_at)
            })
            .await
            .unwrap();
        booked.push(booking);
    }

    let settings = InMemoryNotificationSettingsRepository::default();
    let notifications = InMemoryNotificationRepository::default();
    let announcements = InMemoryAnnouncementRepository::default();
    let notices = FixedCoachLeaveNotices::default();
    let clock = ManualClock::new(booked_at);
    let router = interface::notifications_router(
        ListMyNotifications::new(
            notifications.clone(),
            announcements.clone(),
            notices.clone(),
        ),
        MarkMyNotificationsRead::new(notifications.clone(), announcements, notices, clock.clone()),
        FakeTokens,
    )
    .merge(interface::notification_settings_router(
        GetNotificationSettings::new(settings.clone()),
        UpdateNotificationSettings::new(settings.clone()),
        FakeTokens,
    ));
    let reminders = SendLessonReminders::new(
        SchedulingLessonReminderTargetLookup::new(bookings.clone()),
        settings,
        notifications,
        RecordingPushSender::default(),
        clock.clone(),
    );
    Fixture {
        router,
        bookings,
        clock,
        reminders,
        booked,
    }
}

impl Fixture {
    /// 時計を `now` に進めて、定期の処理を 1 回回す。新しく作ったお知らせの数を返す。
    async fn run_at(&self, now: SystemTime) -> usize {
        self.clock.set(now);
        self.reminders.execute().await.unwrap()
    }

    fn booking_id(&self, student: &str) -> String {
        let booking = self
            .booked
            .iter()
            .find(|booking| booking.student_id() == account(student))
            .unwrap();
        booking_id_uuid::format(booking.id())
    }

    /// `token` のお知らせの (種類, 本文, 予約) を新しい順に。
    async fn reminders_of(&self, token: &str) -> Vec<(String, String, String)> {
        let (status, body) = send(&self.router, "GET", "/notifications/me", token, None).await;
        assert_eq!(status, StatusCode::OK);
        body["notifications"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                (
                    n["kind"].as_str().unwrap().to_string(),
                    n["body"].as_str().unwrap().to_string(),
                    n["booking_id"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    async fn cancel(&self, booking_id: BookingId, by_headquarters: bool, now: SystemTime) {
        self.bookings
            .update(booking_id, |booking, slot| {
                let student = booking.student_id();
                if by_headquarters {
                    booking.cancel_by_headquarters(slot, now).map(|_| ())
                } else {
                    booking.cancel(student, slot, now, true).map(|_| ())
                }
            })
            .await
            .unwrap();
    }
}

/// `token` で呼び、レスポンスを呼んだ人のアプリの契約で検査する（コーチは admin-web、生徒は student-app）。
async fn send(
    router: &Router,
    method: &str,
    path: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    };
    let response = router.clone().oneshot(request.unwrap()).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    let app = if token.starts_with("coach:") {
        "admin-web"
    } else {
        "student-app"
    };
    support::Contract::load(app).assert_response(&method.to_lowercase(), path, status, &body);
    (status, body)
}

fn reminder(body: &str, booking_id: &str) -> (String, String, String) {
    (
        "lesson_reminder".to_string(),
        body.to_string(),
        booking_id.to_string(),
    )
}

const STUDENT_DAY_BEFORE: &str = "予約したレッスンが近づいています";
const STUDENT_HOUR_BEFORE: &str = "予約したレッスンが 1 時間以内に始まります";
const COACH_DAY_BEFORE: &str = "担当するレッスンが近づいています";
const COACH_HOUR_BEFORE: &str = "担当するレッスンが 1 時間以内に始まります";

#[tokio::test]
async fn the_lesson_reminder_setting_reads_true_by_default_and_can_be_turned_off() {
    let f = fixture().await;
    let (status, body) = send(
        &f.router,
        "GET",
        "/notification-settings/me",
        OPTED_OUT_STUDENT,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["lesson_reminder"], json!(true), "既定はオン");

    let (status, body) = send(
        &f.router,
        "PUT",
        "/notification-settings/me",
        OPTED_OUT_STUDENT,
        Some(json!({"lesson_reminder": false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["lesson_reminder"], json!(false));
    assert_eq!(
        body["booking_confirmed"],
        json!(true),
        "書かなかった種類はそのまま"
    );

    let (_, body) = send(
        &f.router,
        "PUT",
        "/notification-settings/me",
        OPTED_OUT_STUDENT,
        Some(json!({"message_received": false})),
    )
    .await;
    assert_eq!(
        body["lesson_reminder"],
        json!(false),
        "書かなければ今のまま"
    );

    let (status, _) = send(
        &f.router,
        "PUT",
        "/notification-settings/me",
        OPTED_OUT_STUDENT,
        Some(json!({"lesson_reminder": "no"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn advancing_the_clock_delivers_one_reminder_per_window_to_the_student_and_the_coach() {
    let f = fixture().await;
    let (status, _) = send(
        &f.router,
        "PUT",
        "/notification-settings/me",
        OPTED_OUT_STUDENT,
        Some(json!({"lesson_reminder": false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mine = f.booking_id(STUDENT);
    let cancelling = f.booking_id(CANCELLING_STUDENT);
    let opted_out = f.booking_id(OPTED_OUT_STUDENT);
    let on_leave = f.booking_id(ON_LEAVE_STUDENT);

    // 前日の窓の前には何も作らない
    assert_eq!(f.run_at(start() - HOUR * 25).await, 0);
    assert!(f.reminders_of(STUDENT).await.is_empty());
    assert!(f.reminders_of(COACH).await.is_empty());

    // コーチのお休みで本部がキャンセルした予約は、前日の窓より前に消える
    f.cancel(f.booked[3].id(), true, start() - HOUR * 30).await;

    // 前日: 生徒 3 人（切った生徒を除く）とコーチに 3 件
    assert_eq!(f.run_at(start() - HOUR * 24).await, 5);
    // 同じ窓で何度回しても増えない
    for hours_later in [0, 1, 12, 22] {
        assert_eq!(f.run_at(start() - HOUR * 24 + HOUR * hours_later).await, 0);
    }
    assert_eq!(
        f.reminders_of(STUDENT).await,
        [reminder(STUDENT_DAY_BEFORE, &mine)]
    );
    assert_eq!(
        f.reminders_of(CANCELLING_STUDENT).await,
        [reminder(STUDENT_DAY_BEFORE, &cancelling)]
    );
    assert!(
        f.reminders_of(OPTED_OUT_STUDENT).await.is_empty(),
        "切った生徒には作らない"
    );
    assert!(
        f.reminders_of(ON_LEAVE_STUDENT).await.is_empty(),
        "お休みで消えた予約には作らない"
    );
    let mut coach = f.reminders_of(COACH).await;
    coach.sort();
    let mut expected = vec![
        reminder(COACH_DAY_BEFORE, &mine),
        reminder(COACH_DAY_BEFORE, &cancelling),
        reminder(COACH_DAY_BEFORE, &opted_out),
    ];
    expected.sort();
    assert_eq!(coach, expected, "コーチには切った生徒の予約の分も作る");
    assert!(!coach.iter().any(|(_, _, booking)| booking == &on_leave));

    // 生徒が 1 時間前の窓の前にキャンセルした予約には、1 時間前のリマインドを作らない
    f.cancel(f.booked[1].id(), false, start() - HOUR * 2).await;

    // 1 時間前: 確定のままの 2 件の予約について、生徒 1 人とコーチに 2 件
    assert_eq!(f.run_at(start() - HOUR).await, 3);
    assert_eq!(f.run_at(start() - HOUR / 2).await, 0, "同じ窓で増えない");
    assert_eq!(
        f.reminders_of(STUDENT).await,
        [
            reminder(STUDENT_HOUR_BEFORE, &mine),
            reminder(STUDENT_DAY_BEFORE, &mine)
        ]
    );
    assert_eq!(
        f.reminders_of(CANCELLING_STUDENT).await,
        [reminder(STUDENT_DAY_BEFORE, &cancelling)],
        "キャンセルした後は増えない"
    );
    assert!(f.reminders_of(OPTED_OUT_STUDENT).await.is_empty());
    let coach = f.reminders_of(COACH).await;
    assert_eq!(coach.len(), 5);
    let mut hour_before: Vec<_> = coach
        .into_iter()
        .filter(|(_, body, _)| body == COACH_HOUR_BEFORE)
        .collect();
    hour_before.sort();
    let mut expected = vec![
        reminder(COACH_HOUR_BEFORE, &mine),
        reminder(COACH_HOUR_BEFORE, &opted_out),
    ];
    expected.sort();
    assert_eq!(hour_before, expected);

    // 始まった後は作らない
    assert_eq!(f.run_at(start()).await, 0);
    assert_eq!(f.run_at(start() + HOUR).await, 0);
}
