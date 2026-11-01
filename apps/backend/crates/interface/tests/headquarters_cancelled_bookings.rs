//! `GET /bookings/cancelled`（admin-web の `listCancelledBookings`。本部が見るキャンセルされた予約の一覧）の統合テスト。
//! レスポンスの形を admin-web の OpenAPI のスキーマで検査し、本部だけが呼べること（403）、キャンセルした日付（日本時間）で
//! 絞ってキャンセルの新しい順に返すこと（枠の開始の日付は問わない）、キャンセルした人が載ること、日付を省いたときは今日までの
//! 7 日であること、期間の違反と日付でない値が 422 であること、同じ階層の `GET /bookings/{booking_id}` と並べても固定の
//! パスが当たることを確かめる。期間の規則の境界は domain の `BookingPeriod`、並べ方の細部はユースケース
//! `ListCancelledBookings` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    identity::{AccountRepository, InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryBookingRepository, InMemorySlotRepository,
    },
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
    scheduling::{BookingRepository, GetBooking, ListCancelledBookings, booking_id_uuid},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
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
const DAY: Duration = Duration::from_secs(24 * 3600);

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
const PATH: &str = "/bookings/cancelled";

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
        (43, "いとう", Role::Coach, vec![Sport::Tennis]),
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

fn slot(id: u128, coach: u128, starts_at: SystemTime) -> Slot {
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
}

/// 生徒 `student` が枠 `slot` を予約し、`cancelled_at` があればその時刻にキャンセルする。
async fn book(
    bookings: &InMemoryBookingRepository,
    student: u128,
    slot: u128,
    cancelled_at: Option<SystemTime>,
) -> Booking {
    let student = AccountId::from_u128(student);
    let booking = bookings
        .reserve(SlotId::from_u128(slot), student, |slot, existing| {
            Booking::reserve(
                bookings.next_id(),
                student,
                false,
                slot,
                existing,
                nov1(Duration::ZERO) - 10 * DAY,
            )
        })
        .await
        .unwrap();
    let Some(cancelled_at) = cancelled_at else {
        return booking;
    };
    bookings
        .update(booking.id(), |booking, slot| {
            booking.cancel(student, slot, cancelled_at).map(|_| ())
        })
        .await
        .unwrap()
}

/// コーチ「たなか」（42）・「いとう」（43）・アカウントのないコーチ（99）の枠へ、生徒「あおき」（3）とアカウントのない生徒（4）が
/// 予約し、日本時間の 10 月 31 日 23 時〜11 月 8 日 0 時にキャンセルした。枠の開始（11 月 20 日〜）は期間の外。
async fn bookings() -> (InMemoryBookingRepository, Vec<Booking>) {
    let later = nov1(19 * DAY);
    let slots = InMemorySlotRepository::with_slots([
        slot(1, 42, later),
        slot(2, 42, later + HOUR),
        slot(3, 43, later + 2 * HOUR),
        slot(4, 99, later + 3 * HOUR),
        slot(5, 42, later + 4 * HOUR),
        slot(6, 43, later + 5 * HOUR),
        slot(7, 42, later + 6 * HOUR),
    ]);
    let bookings = InMemoryBookingRepository::new(slots);
    let made = vec![
        book(&bookings, 3, 1, Some(nov1(Duration::ZERO) - HOUR)).await,
        book(&bookings, 3, 2, Some(nov1(10 * HOUR))).await,
        book(&bookings, 4, 3, Some(nov1(DAY + 9 * HOUR))).await,
        book(&bookings, 3, 4, Some(nov1(DAY + 8 * HOUR))).await,
        book(&bookings, 3, 5, Some(nov1(7 * DAY))).await,
        book(&bookings, 3, 6, Some(nov1(7 * DAY - HOUR))).await,
        // キャンセルしていない予約は出ない
        book(&bookings, 3, 7, None).await,
    ];
    (bookings, made)
}

async fn app_at(now: SystemTime, bookings: InMemoryBookingRepository) -> Router {
    let accounts = accounts().await;
    // 同じ階層の `GET /bookings/{booking_id}` と並べて組み立てる（server と同じ）
    interface::headquarters_cancelled_bookings_router(
        ListCancelledBookings::new(
            bookings.clone(),
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts.clone()),
            FixedClock(now),
        ),
        FakeTokens,
    )
    .merge(interface::headquarters_booking_detail_router(
        GetBooking::new(
            bookings,
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
        ),
        FakeTokens,
    ))
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

fn booking_ids(body: &Value) -> Vec<String> {
    body["bookings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["booking_id"].as_str().unwrap().to_string())
        .collect()
}

fn ids(made: &[Booking], indexes: &[usize]) -> Vec<String> {
    indexes
        .iter()
        .map(|i| booking_id_uuid::format(made[*i].id()))
        .collect()
}

#[tokio::test]
async fn headquarters_lists_bookings_cancelled_in_the_days_newest_first() {
    let admin = support::Contract::load("admin-web");
    let (bookings, made) = bookings().await;
    let app = app_at(nov1(10 * DAY), bookings).await;

    let (status, body) = get(
        &app,
        &format!("{PATH}?from=2026-11-01&to=2026-11-02"),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-11-01"), &json!("2026-11-02"))
    );
    assert_eq!(
        booking_ids(&body),
        ids(&made, &[2, 3, 1]),
        "日本時間の 11 月 1 日〜2 日にキャンセルした予約だけを、キャンセルの新しい順に: {body}"
    );

    let rows = body["bookings"].as_array().unwrap();
    // いとうの枠を、アカウントのない生徒がキャンセルした
    assert_eq!(rows[0]["coach_display_name"], "いとう");
    assert!(rows[0].get("student_display_name").is_none(), "{body}");
    // アカウントのないコーチの枠
    assert!(rows[1].get("coach_display_name").is_none(), "{body}");
    assert_eq!(rows[1]["student_display_name"], "あおき");
    // たなかの枠を、あおきがキャンセルした
    assert_eq!(
        rows[2],
        json!({
            "booking_id": booking_id_uuid::format(made[1].id()),
            "student_id": "00000000-0000-0000-0000-000000000003",
            "student_display_name": "あおき",
            "coach_id": "00000000-0000-0000-0000-00000000002a",
            "coach_display_name": "たなか",
            "starts_at": "2026-11-19T16:00:00Z",
            "cancelled_at": "2026-11-01T01:00:00Z",
            "cancelled_by": "student",
        })
    );
}

#[tokio::test]
async fn without_dates_lists_seven_days_up_to_today_in_japan() {
    let admin = support::Contract::load("admin-web");
    let (bookings, made) = bookings().await;
    // 日本時間の 11 月 7 日 8 時（UTC ではまだ 11 月 6 日）
    let app = app_at(nov1(6 * DAY + 8 * HOUR), bookings).await;

    let (status, body) = get(&app, PATH, HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-11-01"), &json!("2026-11-07"))
    );
    assert_eq!(
        booking_ids(&body),
        ids(&made, &[5, 2, 3, 1]),
        "10 月 31 日 23 時と 11 月 8 日 0 時のキャンセルは入らない: {body}"
    );

    // from だけなら from から 7 日
    let (status, body) = get(&app, &format!("{PATH}?from=2026-11-02"), HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-11-02"), &json!("2026-11-08"))
    );
    assert_eq!(booking_ids(&body), ids(&made, &[4, 5, 2, 3]));

    // to だけなら to までの 7 日
    let (status, body) = get(&app, &format!("{PATH}?to=2026-11-06"), HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-10-31"), &json!("2026-11-06"))
    );
    assert_eq!(booking_ids(&body), ids(&made, &[2, 3, 1, 0]));
}

#[tokio::test]
async fn days_without_cancellations_are_an_empty_list() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;

    // 枠の開始する日付（11 月 20 日）ではキャンセルした予約は拾わない
    let (status, body) = get(
        &app,
        &format!("{PATH}?from=2026-11-20&to=2026-11-20"),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(body["bookings"], json!([]));
}

#[tokio::test]
async fn coach_id_narrows_to_the_coach_keeping_the_days_and_the_order() {
    let admin = support::Contract::load("admin-web");
    let (bookings, made) = bookings().await;
    let app = app_at(nov1(10 * DAY), bookings).await;
    let days = "from=2026-10-31&to=2026-11-08";

    for (coach_id, expected) in [
        // たなか（42）の枠。期間とキャンセルの新しい順は絞らないときと同じ
        ("00000000-0000-0000-0000-00000000002a", vec![4, 1, 0]),
        // いとう（43）の枠
        ("00000000-0000-0000-0000-00000000002b", vec![5, 2]),
        // アカウントのないコーチ（99）でも枠のコーチの ID で絞る
        ("00000000-0000-0000-0000-000000000063", vec![3]),
        // コーチでない ID（生徒のあおき）・ない ID は 404 にせず空の配列
        ("00000000-0000-0000-0000-000000000003", vec![]),
        ("00000000-0000-0000-0000-0000000003e7", vec![]),
    ] {
        let (status, body) = get(
            &app,
            &format!("{PATH}?{days}&coach_id={coach_id}"),
            HEADQUARTERS,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{coach_id}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(
            (&body["from"], &body["to"]),
            (&json!("2026-10-31"), &json!("2026-11-08"))
        );
        assert_eq!(
            booking_ids(&body),
            ids(&made, &expected),
            "{coach_id}: {body}"
        );
        assert!(
            body["bookings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["coach_id"] == coach_id),
            "{body}"
        );
    }

    // 期間を省いても期間の規則は同じ（今日までの 7 日）
    let (status, body) = get(
        &app,
        &format!("{PATH}?coach_id=00000000-0000-0000-0000-00000000002a"),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-11-05"), &json!("2026-11-11"))
    );
    assert_eq!(booking_ids(&body), ids(&made, &[4]));
}

#[tokio::test]
async fn only_headquarters_lists_cancelled_bookings() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;
    for token in [COACH, STUDENT] {
        let (status, body) = get(&app, PATH, token).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        admin.assert_response("get", PATH, status, &body);
    }
    let (status, body) = get(&app, PATH, "nope").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    admin.assert_response("get", PATH, status, &body);
}

#[tokio::test]
async fn invalid_days_are_422() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;
    for (query, message) in [
        (
            "?from=2026-11-02&to=2026-11-01",
            "to は from と同じ日か、それより後の日付にする",
        ),
        // 11 月 1 日〜12 月 2 日は 32 日
        (
            "?from=2026-11-01&to=2026-12-02",
            "from から to までは 31 日まで",
        ),
        ("?from=2026-11-31", "from は YYYY-MM-DD の日付でない"),
        ("?from=2026/11/01", "from は YYYY-MM-DD の日付でない"),
        ("?to=yesterday", "to は YYYY-MM-DD の日付でない"),
        // coach_id の検証は listBookings と同じ
        ("?coach_id=42", "coach_id が UUID でない"),
        ("?coach_id=", "coach_id が UUID でない"),
    ] {
        let (status, body) = get(&app, &format!("{PATH}{query}"), HEADQUARTERS).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(body["code"], "validation_failed", "{query}");
        assert_eq!(body["message"], message, "{query}");
    }
    // 31 日までは読める
    let (status, body) = get(
        &app,
        &format!("{PATH}?from=2026-11-01&to=2026-12-01"),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn booking_detail_on_the_same_level_still_reads_a_booking() {
    let (bookings, made) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;
    let id = booking_id_uuid::format(made[1].id());
    let (status, body) = get(&app, &format!("/bookings/{id}"), HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["booking"]["booking_id"], id);
}
