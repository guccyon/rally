//! `GET /bookings`（admin-web の `listBookings`。本部が見る予約の一覧）の統合テスト。レスポンスの形を admin-web の
//! OpenAPI のスキーマで検査し、本部だけが呼べること（403）、日付（日本時間）で絞って開始の古い順に返すこと、
//! 日付を省いたときは今日から 7 日であること、`coach_id` でそのコーチの予約だけに絞れること（いないコーチは空の一覧）、
//! `student_id` でその生徒の予約だけに絞れること（いない生徒は空の一覧）、
//! `status`（繰り返せる）でその状態の予約だけに絞れること、`trial` で体験予約だけ・体験予約でない予約だけに絞れること、
//! 期間の違反と日付でない値・UUID でない `coach_id`・`student_id`・予約の状態でない `status`・真偽値でない `trial` が 422 であることを
//! 確かめる。期間の規則の境界は
//! domain の `BookingPeriod`、並べ方の細部はユースケース `ListBookings` のテスト。

use std::time::{Duration, SystemTime};

use application::{
    identity::{AccountRepository, InvalidToken, TokenClaims, TokenVerifier},
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryBookingRepository, InMemorySlotRepository,
    },
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
    scheduling::{BookingRepository, ListBookings},
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
    scheduling::{Booking, BookingLessonMenu, Slot, SlotId, SlotSeats, SlotStatus},
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
const PATH: &str = "/bookings";

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

/// 生徒 `student` が枠 `slot` を予約する。`with_menu` なら予約した時点のメニューの写しを残す。
async fn book(
    bookings: &InMemoryBookingRepository,
    student: u128,
    slot: u128,
    is_trial: bool,
    with_menu: bool,
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
                nov1(Duration::ZERO) - DAY,
            )
            .map(|booking| {
                booking.with_restored_lesson_menu(
                    with_menu.then(|| BookingLessonMenu::copy_of(&menu())),
                )
            })
        })
        .await
        .unwrap()
}

/// 日本時間の 11 月 1 日と 2 日に、コーチ「たなか」（42）・「いとう」（43）・アカウントのないコーチ（99）の枠へ
/// 生徒「あおき」（3）とアカウントのない生徒（4）が予約した。期間の境目の前後（10 月 31 日 23 時・11 月 8 日 0 時）にもある。
async fn bookings() -> (InMemoryBookingRepository, Vec<Booking>) {
    let slots = InMemorySlotRepository::with_slots([
        slot(1, 42, nov1(Duration::ZERO) - HOUR),
        slot(2, 42, nov1(10 * HOUR)),
        slot(3, 43, nov1(9 * HOUR)),
        slot(4, 99, nov1(DAY + 8 * HOUR)),
        slot(5, 42, nov1(7 * DAY)),
        slot(6, 43, nov1(7 * DAY - HOUR)),
    ]);
    let bookings = InMemoryBookingRepository::new(slots);
    let made = vec![
        // 体験予約はその生徒の初回の予約だけなので先に
        book(&bookings, 3, 2, true, true).await,
        book(&bookings, 3, 1, false, false).await,
        book(&bookings, 4, 3, false, false).await,
        book(&bookings, 3, 4, false, false).await,
        book(&bookings, 3, 5, false, false).await,
        book(&bookings, 3, 6, false, false).await,
    ];
    (bookings, made)
}

async fn app_at(now: SystemTime, bookings: InMemoryBookingRepository) -> Router {
    let accounts = accounts().await;
    interface::headquarters_bookings_router(
        ListBookings::new(
            bookings,
            ProfileCoachDisplayNameLookup::new(accounts.clone()),
            ProfileStudentDisplayNameLookup::new(accounts),
            FixedClock(now),
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

fn slot_ids(body: &Value) -> Vec<String> {
    body["bookings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["booking"]["slot_id"].as_str().unwrap().to_string())
        .collect()
}

fn slot_uuid(id: u128) -> String {
    format!("00000000-0000-0000-0000-{id:012x}")
}

fn account_uuid(id: u128) -> String {
    format!("00000000-0000-0000-0000-{id:012x}")
}

#[tokio::test]
async fn headquarters_lists_bookings_starting_in_the_days_oldest_first() {
    let admin = support::Contract::load("admin-web");
    let (bookings, made) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;

    let (status, body) = get(
        &app,
        "/bookings?from=2026-11-01&to=2026-11-02",
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
        slot_ids(&body),
        [slot_uuid(3), slot_uuid(2), slot_uuid(4)],
        "日本時間の 11 月 1 日〜2 日に開始する予約だけを、開始の古い順に: {body}"
    );

    let rows = body["bookings"].as_array().unwrap();
    // いとうの枠に、アカウントのない生徒が予約した
    assert_eq!(rows[0]["coach_display_name"], "いとう");
    assert!(rows[0].get("student_display_name").is_none(), "{body}");
    // たなかの枠への体験予約。予約した時点のメニューが載る
    let booking = &made[0];
    assert_eq!(
        rows[1],
        json!({
            "booking": {
                "booking_id": application::scheduling::booking_id_uuid::format(booking.id()),
                "slot_id": slot_uuid(2),
                "student_id": "00000000-0000-0000-0000-000000000003",
                "coach_id": "00000000-0000-0000-0000-00000000002a",
                "is_trial": true,
                "status": "pending",
                "starts_at": "2026-11-01T01:00:00Z",
                "created_at": "2026-10-30T15:00:00Z",
                "confirmed_at": null,
                "cancelled_at": null,
                "lesson_menu": {
                    "lesson_menu_id": "00000000-0000-0000-0000-000000000009",
                    "name": "フォアの基本",
                    "duration_minutes": 60,
                },
            },
            "coach_display_name": "たなか",
            "student_display_name": "あおき",
        })
    );
    // アカウントのないコーチの枠。メニューのない予約は null
    assert!(rows[2].get("coach_display_name").is_none(), "{body}");
    assert_eq!(rows[2]["student_display_name"], "あおき");
    assert_eq!(rows[2]["booking"]["lesson_menu"], Value::Null);
    assert_eq!(rows[2]["booking"]["is_trial"], false);
}

#[tokio::test]
async fn without_dates_lists_seven_days_from_today_in_japan() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    // 日本時間の 11 月 1 日 8 時（UTC ではまだ 10 月 31 日）
    let app = app_at(nov1(8 * HOUR), bookings).await;

    let (status, body) = get(&app, PATH, HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-11-01"), &json!("2026-11-07"))
    );
    assert_eq!(
        slot_ids(&body),
        [slot_uuid(3), slot_uuid(2), slot_uuid(4), slot_uuid(6)],
        "10 月 31 日 23 時と 11 月 8 日 0 時の枠は入らない: {body}"
    );

    // from だけなら from から 7 日
    let (status, body) = get(&app, "/bookings?from=2026-11-02", HEADQUARTERS).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-11-02"), &json!("2026-11-08"))
    );
    assert_eq!(slot_ids(&body), [slot_uuid(4), slot_uuid(6), slot_uuid(5)]);
}

#[tokio::test]
async fn days_without_bookings_are_an_empty_list() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;

    let (status, body) = get(
        &app,
        "/bookings?from=2026-12-01&to=2026-12-31",
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(body["bookings"], json!([]));
}

#[tokio::test]
async fn coach_id_lists_only_bookings_of_that_coach() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;

    let (status, body) = get(
        &app,
        &format!("{PATH}?coach_id={}", account_uuid(42)),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-11-01"), &json!("2026-11-07"))
    );
    assert_eq!(
        slot_ids(&body),
        [slot_uuid(2)],
        "たなかの枠で、期間（今日から 7 日）に開始する予約だけ: {body}"
    );
    assert_eq!(body["bookings"][0]["coach_display_name"], "たなか");

    // 期間と組み合わせる
    let (status, body) = get(
        &app,
        &format!(
            "{PATH}?from=2026-11-01&to=2026-11-08&coach_id={}",
            account_uuid(43)
        ),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(slot_ids(&body), [slot_uuid(3), slot_uuid(6)]);

    // コーチでない ID・ない ID は 404 にせず空の一覧
    for coach_id in [account_uuid(3), account_uuid(1000)] {
        let (status, body) = get(&app, &format!("{PATH}?coach_id={coach_id}"), HEADQUARTERS).await;
        assert_eq!(status, StatusCode::OK, "{coach_id}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(body["bookings"], json!([]), "{coach_id}");
    }
}

#[tokio::test]
async fn student_id_lists_only_bookings_of_that_student() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;

    let (status, body) = get(
        &app,
        &format!("{PATH}?student_id={}", account_uuid(3)),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    admin.assert_response("get", PATH, status, &body);
    assert_eq!(
        (&body["from"], &body["to"]),
        (&json!("2026-11-01"), &json!("2026-11-07"))
    );
    assert_eq!(
        slot_ids(&body),
        [slot_uuid(2), slot_uuid(4), slot_uuid(6)],
        "あおきの予約で、期間（今日から 7 日）に開始する予約だけ: {body}"
    );
    assert!(
        body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["student_display_name"] == "あおき"),
        "{body}"
    );

    // 期間と組み合わせる。並べ方は変わらない
    let (status, body) = get(
        &app,
        &format!(
            "{PATH}?from=2026-10-31&to=2026-11-08&student_id={}",
            account_uuid(3)
        ),
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        slot_ids(&body),
        [
            slot_uuid(1),
            slot_uuid(2),
            slot_uuid(4),
            slot_uuid(6),
            slot_uuid(5)
        ]
    );

    // コーチ・状態・体験予約かと組み合わせる
    for (query, expected) in [
        (
            format!(
                "coach_id={}&student_id={}",
                account_uuid(43),
                account_uuid(3)
            ),
            vec![slot_uuid(6)],
        ),
        (
            format!(
                "coach_id={}&student_id={}",
                account_uuid(43),
                account_uuid(4)
            ),
            vec![slot_uuid(3)],
        ),
        (
            format!("student_id={}&trial=true", account_uuid(3)),
            vec![slot_uuid(2)],
        ),
        (
            format!("student_id={}&status=confirmed", account_uuid(3)),
            vec![],
        ),
    ] {
        let (status, body) = get(&app, &format!("{PATH}?{query}"), HEADQUARTERS).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(slot_ids(&body), expected, "{query}: {body}");
    }

    // 生徒でない ID（コーチ）・ない ID は 404 にせず空の一覧
    for student_id in [account_uuid(42), account_uuid(1000)] {
        let (status, body) = get(
            &app,
            &format!("{PATH}?student_id={student_id}"),
            HEADQUARTERS,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{student_id}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(body["bookings"], json!([]), "{student_id}");
    }
}

#[tokio::test]
async fn status_lists_only_bookings_in_those_statuses() {
    let admin = support::Contract::load("admin-web");
    let (bookings, made) = bookings().await;
    // 11 月 1 日〜7 日の予約（枠 3・2・4・6）のうち、枠 2 を確定し、枠 4 をキャンセルする
    let student = AccountId::from_u128(3);
    bookings
        .update(made[0].id(), |booking, slot| {
            booking.confirm(student, slot, nov1(Duration::ZERO) - HOUR)
        })
        .await
        .unwrap();
    bookings
        .update(made[3].id(), |booking, slot| {
            booking
                .cancel(student, slot, nov1(Duration::ZERO) - HOUR)
                .map(|_| ())
        })
        .await
        .unwrap();
    let app = app_at(nov1(Duration::ZERO), bookings).await;

    for (query, expected) in [
        ("?status=pending", vec![slot_uuid(3), slot_uuid(6)]),
        ("?status=confirmed", vec![slot_uuid(2)]),
        ("?status=cancelled", vec![slot_uuid(4)]),
        // 繰り返せば、どれかの状態のもの。並べ方は変わらない
        (
            "?status=cancelled&status=pending",
            vec![slot_uuid(3), slot_uuid(4), slot_uuid(6)],
        ),
        // 同じ値を重ねても同じ
        ("?status=confirmed&status=confirmed", vec![slot_uuid(2)]),
        // 付けなければ状態を問わない（今までと同じ）
        (
            "",
            vec![slot_uuid(3), slot_uuid(2), slot_uuid(4), slot_uuid(6)],
        ),
        // 期間・コーチと組み合わせる
        (
            "?from=2026-11-01&to=2026-11-01&status=pending&status=confirmed",
            vec![slot_uuid(3), slot_uuid(2)],
        ),
        (
            "?coach_id=00000000-0000-0000-0000-00000000002a&status=pending",
            vec![],
        ),
    ] {
        let (status, body) = get(&app, &format!("{PATH}{query}"), HEADQUARTERS).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(slot_ids(&body), expected, "{query}: {body}");
    }
    let (_, body) = get(&app, &format!("{PATH}?status=confirmed"), HEADQUARTERS).await;
    assert_eq!(body["bookings"][0]["booking"]["status"], "confirmed");
}

#[tokio::test]
async fn trial_lists_only_trial_or_only_regular_bookings() {
    let admin = support::Contract::load("admin-web");
    // 11 月 1 日〜7 日の予約は枠 3・2・4・6 で、体験予約は枠 2 だけ
    let (bookings, made) = bookings().await;
    let student = AccountId::from_u128(3);
    bookings
        .update(made[0].id(), |booking, slot| {
            booking
                .cancel(student, slot, nov1(Duration::ZERO) - HOUR)
                .map(|_| ())
        })
        .await
        .unwrap();
    let app = app_at(nov1(Duration::ZERO), bookings).await;

    for (query, expected) in [
        // キャンセルした体験予約も体験予約
        ("?trial=true", vec![slot_uuid(2)]),
        (
            "?trial=false",
            vec![slot_uuid(3), slot_uuid(4), slot_uuid(6)],
        ),
        // 付けなければ体験予約かを問わない（今までと同じ）
        (
            "",
            vec![slot_uuid(3), slot_uuid(2), slot_uuid(4), slot_uuid(6)],
        ),
        // 期間・コーチ・状態と組み合わせる
        (
            "?from=2026-11-02&to=2026-11-07&trial=false",
            vec![slot_uuid(4), slot_uuid(6)],
        ),
        (
            "?coach_id=00000000-0000-0000-0000-00000000002a&trial=true",
            vec![slot_uuid(2)],
        ),
        ("?status=pending&trial=true", vec![]),
        ("?status=cancelled&trial=true", vec![slot_uuid(2)]),
    ] {
        let (status, body) = get(&app, &format!("{PATH}{query}"), HEADQUARTERS).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(slot_ids(&body), expected, "{query}: {body}");
    }
    let (_, body) = get(&app, &format!("{PATH}?trial=true"), HEADQUARTERS).await;
    assert_eq!(body["bookings"][0]["booking"]["is_trial"], true);
    let (_, body) = get(&app, &format!("{PATH}?trial=false"), HEADQUARTERS).await;
    assert!(
        body["bookings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["booking"]["is_trial"] == false)
    );
}

#[tokio::test]
async fn trial_that_is_not_a_boolean_is_422() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;
    for query in ["?trial=yes", "?trial=", "?trial=1", "?trial=TRUE"] {
        let (status, body) = get(&app, &format!("{PATH}{query}"), HEADQUARTERS).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        admin.assert_response("get", PATH, status, &body);
        assert_eq!(body["code"], "validation_failed", "{query}");
    }
}

#[tokio::test]
async fn only_headquarters_lists_bookings() {
    let admin = support::Contract::load("admin-web");
    let (bookings, _) = bookings().await;
    let app = app_at(nov1(Duration::ZERO), bookings).await;
    for token in [COACH, STUDENT] {
        for path in [PATH.to_string(), format!("{PATH}?trial=true")] {
            let (status, body) = get(&app, &path, token).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{token} {path}: {body}");
            admin.assert_response("get", PATH, status, &body);
        }
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
        // 省いた from は今日（11 月 1 日）
        (
            "?to=2026-10-31",
            "to は from と同じ日か、それより後の日付にする",
        ),
        // 11 月 1 日〜12 月 2 日は 32 日
        (
            "?from=2026-11-01&to=2026-12-02",
            "from から to までは 31 日まで",
        ),
        ("?from=2026-11-31", "from は YYYY-MM-DD の日付でない"),
        ("?from=2026/11/01", "from は YYYY-MM-DD の日付でない"),
        ("?to=tomorrow", "to は YYYY-MM-DD の日付でない"),
        ("?coach_id=42", "coach_id が UUID でない"),
        ("?coach_id=", "coach_id が UUID でない"),
        ("?student_id=3", "student_id が UUID でない"),
        ("?student_id=", "student_id が UUID でない"),
        ("?status=canceled", "未知の予約の状態: canceled"),
        (
            "?status=pending&status=PENDING",
            "未知の予約の状態: PENDING",
        ),
        (
            "?status=pending,cancelled",
            "未知の予約の状態: pending,cancelled",
        ),
        ("?status=", "未知の予約の状態: "),
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
        "/bookings?from=2026-11-01&to=2026-12-01",
        HEADQUARTERS,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
