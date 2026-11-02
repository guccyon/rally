//! `GET /bookings/{booking_id}`（admin-web の `getBooking`。本部が見る予約 1 件の詳細）の統合テスト。レスポンスの形を
//! admin-web の OpenAPI のスキーマで検査し、予約・予約したスロット（枠の開始と終わり）・状態の移り変わり・コーチと生徒の
//! 表示名を返すこと、本部だけが呼べること（403）、ない予約と UUID でない ID が 404 であることを確かめる。状態の移り変わりの
//! 組み立ては domain の `Booking::status_changes`、読み取りはユースケース `GetBooking` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    identity::{AccountRepository, InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{InMemoryAccountRepository, InMemoryBookingRepository, InMemorySlotRepository},
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
    scheduling::{BookingRepository, GetBooking, booking_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::{
        LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
        LessonMenuId, LessonMenuName, Sport,
    },
    identity::{Account, AccountId, Email, Role},
    profile::DisplayName,
    scheduling::{Booking, Slot, SlotId, SlotSeats, SlotStatus},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-11-01T00:00:00+09:00（2026-10-31T15:00:00Z）
const NOV1_JST: Duration = Duration::from_secs(1_793_458_800);
const HOUR: Duration = Duration::from_secs(3600);

/// `<役割>:<ID の数値>` の形のトークンを通す偽物（interface は infra のモックに依存できない）。
struct FakeTokens;

impl TokenVerifier for FakeTokens {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        let (role, id) = token.split_once(':').ok_or(InvalidToken)?;
        let role = match role {
            "student" => Role::Student,
            "coach" => Role::Coach,
            "headquarters" => Role::Headquarters,
            _ => return Err(InvalidToken),
        };
        let id = id.parse().map_err(|_| InvalidToken)?;
        Ok(TokenClaims {
            account_id: AccountId::from_u128(id),
            role,
        })
    }
}

const HEADQUARTERS: &str = "headquarters:1";
const COACH: &str = "coach:42";
const STUDENT: &str = "student:3";
const PATH: &str = "/bookings/{booking_id}";

/// 日本時間の 2026 年 11 月 1 日 0 時から `offset` 後
fn nov1(offset: Duration) -> SystemTime {
    SystemTime::UNIX_EPOCH + NOV1_JST + offset
}

async fn accounts() -> InMemoryAccountRepository {
    let accounts = InMemoryAccountRepository::default();
    for (id, name, role, sports) in [
        (1, "本部", Role::Headquarters, vec![]),
        (3, "あおき", Role::Student, vec![Sport::Tennis]),
        (42, "たなか", Role::Coach, vec![Sport::Tennis]),
    ] {
        accounts
            .insert(
                Account::new(
                    AccountId::from_u128(id),
                    Email::new(format!("user{id}@example.com")).unwrap(),
                    DisplayName::new(name).unwrap(),
                    role,
                    sports,
                    nov1(Duration::ZERO),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    accounts
}

fn menu() -> LessonMenu {
    LessonMenu::restore(
        LessonMenuId::from_u128(9),
        AccountId::from_u128(42),
        LessonMenuContent {
            sport: Sport::Tennis,
            format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
            name: LessonMenuName::new("フォアの基本").unwrap(),
            description: LessonMenuDescription::default(),
        },
        true,
        nov1(Duration::ZERO),
    )
}

/// コーチ「たなか」（42）のメニューつきの枠（1）と、アカウントのないコーチ（99）の枠（2）。
fn slots() -> InMemorySlotRepository {
    let slot = |id: u128, coach: u128, starts_at: SystemTime| {
        Slot::restore(
            SlotId::from_u128(id),
            AccountId::from_u128(coach),
            Sport::Tennis,
            starts_at,
            starts_at + HOUR,
            SlotStatus::Open,
            SlotSeats::default(),
        )
        .unwrap()
    };
    InMemorySlotRepository::with_slots([
        slot(1, 42, nov1(10 * HOUR))
            .with_lesson_menu(&menu())
            .unwrap(),
        slot(2, 99, nov1(12 * HOUR)),
    ])
}

/// 生徒 `student` が枠 `slot` を 11 月 1 日 0 時に予約する。
async fn book(
    bookings: &InMemoryBookingRepository,
    student: u128,
    slot: u128,
    is_trial: bool,
) -> Booking {
    let student = AccountId::from_u128(student);
    bookings
        .reserve(SlotId::from_u128(slot), student, |slot, existing| {
            Booking::reserve(
                bookings.next_id(),
                student,
                is_trial,
                slot,
                existing,
                nov1(Duration::ZERO),
            )
            .map(|booking| booking.with_lesson_menu(slot, Some(&menu())))
        })
        .await
        .unwrap()
}

async fn app(bookings: InMemoryBookingRepository) -> Router {
    let accounts = accounts().await;
    interface::headquarters_booking_detail_router(
        GetBooking::new(
            bookings,
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
        ),
        FakeTokens,
    )
}

async fn get(app: &Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let request = Request::get(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn uri(booking: &Booking) -> String {
    format!("/bookings/{}", booking_id_uuid::format(booking.id()))
}

#[tokio::test]
async fn headquarters_reads_a_booking_with_its_slot_and_status_changes() {
    let admin = support::Contract::load("admin-web");
    let bookings = InMemoryBookingRepository::new(slots());
    let booking = book(&bookings, 3, 1, true).await;
    bookings
        .update(booking.id(), |booking, slot| {
            booking.confirm(AccountId::from_u128(3), slot, nov1(HOUR))
        })
        .await
        .unwrap();
    bookings
        .update(booking.id(), |booking, slot| {
            booking
                .cancel(AccountId::from_u128(3), slot, nov1(2 * HOUR))
                .map(|_| ())
        })
        .await
        .unwrap();
    let app = app(bookings).await;

    let (status, body) = get(&app, &uri(&booking), HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(
        body,
        json!({
            "booking": {
                "booking_id": booking_id_uuid::format(booking.id()),
                "slot_id": "00000000-0000-0000-0000-000000000001",
                "student_id": "00000000-0000-0000-0000-000000000003",
                "coach_id": "00000000-0000-0000-0000-00000000002a",
                "is_trial": true,
                "status": "cancelled",
                "starts_at": "2026-11-01T01:00:00Z",
                "created_at": "2026-10-31T15:00:00Z",
                "confirmed_at": "2026-10-31T16:00:00Z",
                "cancelled_at": "2026-10-31T17:00:00Z",
                "lesson_menu": {
                    "lesson_menu_id": "00000000-0000-0000-0000-000000000009",
                    "name": "フォアの基本",
                    "duration_minutes": 60,
                },
            },
            "slot": {
                "slot_id": "00000000-0000-0000-0000-000000000001",
                "coach_id": "00000000-0000-0000-0000-00000000002a",
                "sport": "tennis",
                "starts_at": "2026-11-01T01:00:00Z",
                "ends_at": "2026-11-01T02:00:00Z",
                "status": "open",
                "capacity": 1,
                "remaining_seats": 1,
            },
            "status_changes": [
                { "status": "confirmed", "changed_at": "2026-10-31T16:00:00Z" },
                { "status": "cancelled", "changed_at": "2026-10-31T17:00:00Z" },
            ],
            "coach_display_name": "たなか",
            "student_display_name": "あおき",
        })
    );
}

#[tokio::test]
async fn pending_booking_has_no_status_changes_and_unknown_accounts_have_no_names() {
    let admin = support::Contract::load("admin-web");
    let bookings = InMemoryBookingRepository::new(slots());
    // アカウントのない生徒（4）が、アカウントのないコーチ（99）の枠を予約した
    let booking = book(&bookings, 4, 2, false).await;
    let app = app(bookings).await;

    let (status, body) = get(&app, &uri(&booking), HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(body["booking"]["status"], "pending");
    assert_eq!(body["booking"]["is_trial"], false);
    assert_eq!(
        body["booking"]["lesson_menu"],
        Value::Null,
        "枠にメニューがない"
    );
    assert_eq!(body["slot"]["ends_at"], "2026-11-01T04:00:00Z");
    assert_eq!(body["status_changes"], json!([]));
    assert!(body.get("coach_display_name").is_none(), "{body}");
    assert!(body.get("student_display_name").is_none(), "{body}");
}

#[tokio::test]
async fn unknown_or_malformed_booking_is_not_found() {
    let admin = support::Contract::load("admin-web");
    let bookings = InMemoryBookingRepository::new(slots());
    book(&bookings, 3, 1, false).await;
    let app = app(bookings).await;
    for uri in [
        "/bookings/00000000-0000-0000-0000-000000000063",
        "/bookings/not-a-uuid",
    ] {
        let (status, body) = get(&app, uri, HEADQUARTERS).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(body["code"], "not_found");
    }
}

#[tokio::test]
async fn only_headquarters_reads_a_booking() {
    let admin = support::Contract::load("admin-web");
    let bookings = InMemoryBookingRepository::new(slots());
    let booking = book(&bookings, 3, 1, false).await;
    let app = app(bookings).await;
    // 予約した生徒本人と枠のコーチでも 403
    for token in [COACH, STUDENT] {
        let (status, body) = get(&app, &uri(&booking), token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        admin.assert_response("get", PATH, status, &body);
    }
    let (status, body) = get(&app, &uri(&booking), "nope").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    admin.assert_response("get", PATH, status, &body);
}
