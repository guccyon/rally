//! `GET /students`（admin-web の `listStudents`）の統合テスト。本部には生徒の一覧を、受講権の契約種別と契約プラン・
//! 最後に予約した時刻・直近 30 日の予約の数とともに返すことを OpenAPI のスキーマで検査する。コーチ・生徒は 403、トークンがなければ 401。
//! 検索語 `q` を付ければ表示名に部分一致する生徒だけを返し、長すぎる `q` は 422。`sport`・`not_booked_since` を付ければ
//! そのスポーツの・その日（日本時間）以降に予約していない生徒だけを返し、値が不正なら 422。`coach_id` を付ければ
//! そのコーチを予約したことがある生徒だけを返し（ない・コーチでない ID は空の一覧）、UUID でなければ 422。
//! `has_unanswered=true` を付ければ返信のないまま既定の時間の過ぎたスレッドのある生徒だけを返し、真偽でなければ 422。
//! `has_pending_video=true` を付ければ返却待ちの動画のある生徒だけを返し、真偽でなければ 422。
//! `has_unhandled_low_rated=true` を付ければ本部が未対応の直近の低い評価をつけた生徒だけを返し、真偽でなければ 422。
//! この操作はパスパラメータを受けないので、404（見つからない）の応答はない。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::AsyncLessonStudentVideoActivityLookup,
    entitlement::EntitlementStudentContractLookup,
    identity::{
        AccessToken, AccountRepository, InvalidToken, ListStudents, TokenClaims, TokenIssuer,
        TokenVerifier,
    },
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryBookingRepository,
        InMemoryEntitlementRepository, InMemoryLessonReviewRepository,
        InMemoryMessageThreadRepository, InMemorySlotRepository, InMemoryVideoRepository,
    },
    messaging::MessagingStudentUnansweredMessageThreadCountLookup,
    review::ReviewStudentLowRatedReviewCountLookup,
    scheduling::{
        BookingRepository, SchedulingStudentCoachBookingLookup, SchedulingStudentLastBookingLookup,
        SchedulingStudentRecentBookingCountLookup,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
    catalog::Sport,
    entitlement::{
        Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan, YearMonth,
    },
    identity::{Account, AccountId, Email, Role},
    lesson::LessonCompletionId,
    messaging::{MessageBody, MessageContent, MessageId, MessageThread, MessageThreadId},
    profile::DisplayName,
    review::{LessonReview, LessonReviewComment, LessonReviewId, LowRatedReviewHandling, Rating},
    scheduling::{Booking, Slot, SlotId, SlotSeats, SlotStatus},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

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
const COACH: AccountId = AccountId::from_u128(2);
/// テニスの月額（plus）の生徒。予約が 2 件ある
const MONTHLY_STUDENT: AccountId = AccountId::from_u128(3);
/// ゴルフのワンショットの生徒
const GOLF_STUDENT: AccountId = AccountId::from_u128(4);
/// 受講権も予約もない水泳の生徒
const NEW_STUDENT: AccountId = AccountId::from_u128(5);

/// 2026-10-01T00:00:00Z から `hour` 時間後。
fn at(hour: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_812_800 + hour * 3600)
}

fn account(id: AccountId, role: Role, sports: &[Sport], registered_hour: u64) -> Account {
    Account::new(
        id,
        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
        DisplayName::new(format!("なまえ{}", id.as_u128())).unwrap(),
        role,
        sports.iter().copied(),
        at(registered_hour),
    )
    .unwrap()
}

fn slot(id: u128, hour: u64) -> Slot {
    Slot::restore(
        SlotId::from_u128(id),
        COACH,
        Sport::Tennis,
        at(hour),
        at(hour + 1),
        SlotStatus::Open,
        SlotSeats::default(),
    )
    .unwrap()
}

/// 照会した時刻（2026-10-02T00:00:00Z）。月額の生徒の 2 件の予約はどちらも直近 30 日に入る。
fn now() -> SystemTime {
    at(24)
}

/// 本部・コーチと、生徒 3 人（登録は月額の生徒 → ゴルフの生徒 → 新しい生徒の順）。月額の生徒は 2 回予約した。
/// 月額の生徒はコーチに返信をもらっていないスレッドが、ゴルフの生徒はコーチが返したスレッドがある（[`threads`]）。
/// 月額の生徒と新しい生徒は返却待ちの動画が、ゴルフの生徒は返却済みの動画だけがある（[`videos`]）。
/// 新しい生徒とゴルフの生徒は本部が未対応の低い評価を、月額の生徒は対応済みの低い評価と数えない評価だけをつけた
/// （[`reviews`]）。照会した時刻は [`now`]。
async fn app() -> Router {
    app_at(now()).await
}

/// [`app`] と同じ生徒と予約で、照会した時刻を `now` にしたもの。
async fn app_at(now: SystemTime) -> Router {
    let accounts = InMemoryAccountRepository::default();
    for account in [
        account(AccountId::from_u128(1), Role::Headquarters, &[], 0),
        account(COACH, Role::Coach, &[Sport::Tennis], 0),
        account(MONTHLY_STUDENT, Role::Student, &[Sport::Tennis], 1),
        account(GOLF_STUDENT, Role::Student, &[Sport::Golf], 2),
        account(NEW_STUDENT, Role::Student, &[Sport::Swimming], 3),
    ] {
        accounts.insert(account).await.unwrap();
    }
    let entitlements = InMemoryEntitlementRepository::with_entitlements([
        Entitlement::monthly(
            EntitlementId::from_u128(10),
            MONTHLY_STUDENT,
            Sport::Tennis,
            SubscriptionId::from_u128(10),
            LiveLessonQuota::restore(
                SubscriptionPlan::Plus,
                YearMonth::new(2026, 10).unwrap(),
                [],
            )
            .unwrap(),
        )
        .unwrap(),
        Entitlement::one_shot(EntitlementId::from_u128(11), GOLF_STUDENT, Sport::Golf, 1).unwrap(),
    ]);
    let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
        slot(20, 100),
        slot(21, 50),
    ]));
    for (slot_id, created_hour) in [(20, 10), (21, 12)] {
        bookings
            .reserve(
                SlotId::from_u128(slot_id),
                MONTHLY_STUDENT,
                |slot, existing| {
                    Booking::reserve(
                        bookings.next_id(),
                        MONTHLY_STUDENT,
                        false,
                        slot,
                        existing,
                        at(created_hour),
                    )
                },
            )
            .await
            .unwrap();
    }
    router(
        accounts,
        entitlements,
        bookings,
        threads(),
        videos(),
        reviews(),
        now,
    )
}

/// `id` の `student` とコーチのスレッドを開き、`exchange` の順に（生徒が送ったか, 送信時刻）で本文を送る。
fn thread(id: u128, student: AccountId, exchange: &[(bool, SystemTime)]) -> MessageThread {
    let mut thread =
        MessageThread::open(MessageThreadId::from_u128(id), student, COACH, at(0)).unwrap();
    for (n, (by_student, sent_at)) in exchange.iter().enumerate() {
        thread
            .send(
                MessageId::from_u128(id * 100 + n as u128),
                if *by_student { student } else { COACH },
                MessageContent::Text(MessageBody::new("本部には見せない本文").unwrap()),
                true,
                *sent_at,
            )
            .unwrap();
    }
    thread
}

/// 月額の生徒が [`now`] のちょうど 24 時間前に送ってまだ返信のないスレッドと、ゴルフの生徒が送りコーチが返したスレッド。
fn threads() -> InMemoryMessageThreadRepository {
    InMemoryMessageThreadRepository::with_threads([
        thread(30, MONTHLY_STUDENT, &[(true, at(0))]),
        thread(31, GOLF_STUDENT, &[(true, at(0)), (false, at(1))]),
    ])
}

/// 月額の生徒が送った返却待ちの動画と、新しい生徒が [`now`] より後に送った返却待ちの動画（経過時間を問わず返却待ち）と、
/// ゴルフの生徒が送りコーチが返した動画。
fn videos() -> InMemoryVideoRepository {
    let pending = |id: u128, student: AccountId, sport: Sport, hour: u64| {
        Video::submit(VideoId::from_u128(id), student, COACH, sport, at(hour))
    };
    InMemoryVideoRepository::with_videos([
        pending(40, MONTHLY_STUDENT, Sport::Tennis, 2),
        pending(41, NEW_STUDENT, Sport::Swimming, 30),
        Video::restore(
            VideoId::from_u128(42),
            GOLF_STUDENT,
            COACH,
            Sport::Golf,
            at(2),
            Some((
                VideoReviewId::from_u128(142),
                ReviewComment::new("テイクバックが早い").unwrap(),
                at(3),
            )),
            None,
        )
        .unwrap(),
    ])
}

/// 生徒がコーチにつけた評価。新しい生徒は未対応の ★2、ゴルフの生徒は未対応の ★1。月額の生徒は対応済みの ★1・★4・
/// 31 日前の未対応の ★1（どれも未対応の低い評価に数えない）。
fn reviews() -> InMemoryLessonReviewRepository {
    let review = |id: u128, student: AccountId, rating: i64, submitted_at: SystemTime| {
        LessonReview::restore(
            LessonReviewId::from_u128(id),
            LessonCompletionId::from_u128(id),
            student,
            COACH,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("本部には見せない本文").unwrap(),
            submitted_at,
        )
    };
    InMemoryLessonReviewRepository::with_reviews([
        review(50, NEW_STUDENT, 2, at(20)),
        review(51, GOLF_STUDENT, 1, at(5)),
        review(52, MONTHLY_STUDENT, 1, at(6)).with_handling(Some(LowRatedReviewHandling::new(
            AccountId::from_u128(1),
            at(7),
        ))),
        review(53, MONTHLY_STUDENT, 4, at(8)),
        review(
            54,
            MONTHLY_STUDENT,
            1,
            now() - Duration::from_secs(31 * 24 * 3600),
        ),
    ])
}

#[allow(clippy::too_many_arguments)]
fn router(
    accounts: InMemoryAccountRepository,
    entitlements: InMemoryEntitlementRepository,
    bookings: InMemoryBookingRepository,
    threads: InMemoryMessageThreadRepository,
    videos: InMemoryVideoRepository,
    reviews: InMemoryLessonReviewRepository,
    now: SystemTime,
) -> Router {
    interface::students_router(
        ListStudents::new(
            accounts,
            EntitlementStudentContractLookup::new(entitlements),
            SchedulingStudentLastBookingLookup::new(bookings.clone()),
            SchedulingStudentCoachBookingLookup::new(bookings.clone()),
            SchedulingStudentRecentBookingCountLookup::new(bookings),
            MessagingStudentUnansweredMessageThreadCountLookup::new(threads),
            AsyncLessonStudentVideoActivityLookup::new(videos),
            ReviewStudentLowRatedReviewCountLookup::new(reviews),
            FixedClock(now),
        ),
        FakeTokens,
    )
}

/// `GET /students`。レスポンスを admin-web の契約で検査する。
async fn list_students(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    list_students_at(router, "/students", token).await
}

/// クエリ付きの `GET /students`（`uri` はパスとクエリ）。レスポンスを admin-web の契約で検査する。
async fn list_students_at(router: &Router, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("admin-web").assert_response("get", "/students", status, &body);
    (status, body)
}

#[tokio::test]
async fn headquarters_lists_students_newest_first_with_contract_and_last_booking() {
    let (status, body) = list_students(&app().await, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "students": [
                {
                    "student_id": "00000000-0000-0000-0000-000000000005",
                    "display_name": "なまえ5",
                    "sport": "swimming",
                    "contract_type": null,
                    "plan": null,
                    "last_booked_at": null,
                    "recent_booking_count": 0,
                    "registered_at": "2026-10-01T03:00:00Z",
                },
                {
                    "student_id": "00000000-0000-0000-0000-000000000004",
                    "display_name": "なまえ4",
                    "sport": "golf",
                    "contract_type": "one_shot",
                    "plan": null,
                    "last_booked_at": null,
                    "recent_booking_count": 0,
                    "registered_at": "2026-10-01T02:00:00Z",
                },
                {
                    "student_id": "00000000-0000-0000-0000-000000000003",
                    "display_name": "なまえ3",
                    "sport": "tennis",
                    "contract_type": "subscription",
                    "plan": "plus",
                    // 開始の遅い枠の予約ではなく、後で作った予約の時刻
                    "last_booked_at": "2026-10-01T12:00:00Z",
                    "recent_booking_count": 2,
                    "registered_at": "2026-10-01T01:00:00Z",
                },
            ],
        }),
        "生徒だけを載せ、メールアドレスは載せない"
    );
}

#[tokio::test]
async fn recent_booking_count_counts_bookings_of_the_last_thirty_days_including_the_start() {
    // 月額の生徒の予約は 2026-10-01 の 10 時と 12 時に作った。12 時ちょうどの 30 日後に照会すると、12 時の予約は期間の
    // 始まりちょうどで数え、10 時の予約は期間の外
    let count = |body: &Value| -> Vec<u64> {
        body["students"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["recent_booking_count"].as_u64().unwrap())
            .collect()
    };
    let (status, body) = list_students(&app_at(at(12 + 30 * 24)).await, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(count(&body), [0, 0, 1]);
    assert_eq!(
        body["students"][2]["last_booked_at"], "2026-10-01T12:00:00Z",
        "最後に予約した時刻は期間に関係なく載る"
    );
    let (_, body) = list_students(
        &app_at(at(12 + 30 * 24) + Duration::from_secs(1)).await,
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(count(&body), [0, 0, 0], "30 日より前の予約は数えない");
}

#[tokio::test]
async fn recent_booking_count_leaves_out_cancelled_bookings() {
    let accounts = InMemoryAccountRepository::default();
    accounts
        .insert(account(MONTHLY_STUDENT, Role::Student, &[Sport::Tennis], 1))
        .await
        .unwrap();
    let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
        slot(20, 100),
        slot(21, 50),
    ]));
    let mut made = Vec::new();
    for slot_id in [20, 21] {
        made.push(
            bookings
                .reserve(
                    SlotId::from_u128(slot_id),
                    MONTHLY_STUDENT,
                    |slot, existing| {
                        Booking::reserve(
                            bookings.next_id(),
                            MONTHLY_STUDENT,
                            false,
                            slot,
                            existing,
                            at(10),
                        )
                    },
                )
                .await
                .unwrap(),
        );
    }
    bookings
        .update(made[0].id(), |booking, slot| {
            booking.cancel(MONTHLY_STUDENT, slot, at(11)).map(|_| ())
        })
        .await
        .unwrap();
    let router = router(
        accounts,
        InMemoryEntitlementRepository::default(),
        bookings,
        InMemoryMessageThreadRepository::default(),
        InMemoryVideoRepository::default(),
        InMemoryLessonReviewRepository::default(),
        now(),
    );

    let (status, body) = list_students(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["students"][0]["recent_booking_count"], 1,
        "キャンセルした予約は数えない"
    );
}

/// 生徒の ID の末尾の数（`students` の並び順）。
fn listed_ids(body: &Value) -> Vec<String> {
    body["students"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            s["student_id"]
                .as_str()
                .unwrap()
                .trim_start_matches(['0', '-'])
                .to_string()
        })
        .collect()
}

#[tokio::test]
async fn q_lists_only_students_whose_display_name_contains_it() {
    let router = app().await;
    // 「なまえ」は生徒 3 人とも含む。「%E3%81%BE%E3%81%883」は「まえ3」
    for (uri, expected) in [
        (
            "/students?q=%E3%81%AA%E3%81%BE%E3%81%88",
            vec!["5", "4", "3"],
        ),
        ("/students?q=%E3%81%BE%E3%81%883", vec!["3"]),
        // 前後の空白（全角の空白を含む）は除いて比べる
        ("/students?q=%E3%80%80%E3%81%BE%E3%81%884+", vec!["4"]),
        ("/students?q=%E3%81%84%E3%81%AA%E3%81%84", vec![]),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), expected, "{uri}");
    }
    let (_, body) = list_students_at(
        &router,
        "/students?q=%E3%81%BE%E3%81%883",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(
        body["students"][0]["contract_type"], "subscription",
        "絞り込んでも受講権と最後の予約を添える"
    );
    assert_eq!(
        body["students"][0]["last_booked_at"],
        "2026-10-01T12:00:00Z"
    );
}

#[tokio::test]
async fn q_matches_ascii_letters_case_insensitively() {
    let accounts = InMemoryAccountRepository::default();
    for (id, name) in [(3, "Taro YAMADA"), (4, "ＹＡＭＡＤＡ")] {
        let account = Account::new(
            AccountId::from_u128(id),
            Email::new(format!("user{id}@example.com")).unwrap(),
            DisplayName::new(name).unwrap(),
            Role::Student,
            [Sport::Tennis],
            at(id as u64),
        )
        .unwrap();
        accounts.insert(account).await.unwrap();
    }
    let router = router(
        accounts,
        InMemoryEntitlementRepository::default(),
        InMemoryBookingRepository::new(InMemorySlotRepository::default()),
        InMemoryMessageThreadRepository::default(),
        InMemoryVideoRepository::default(),
        InMemoryLessonReviewRepository::default(),
        now(),
    );
    let (status, body) = list_students_at(&router, "/students?q=yamada", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(listed_ids(&body), ["3"], "全角の英字は同じとみなさない");
}

#[tokio::test]
async fn blank_or_missing_q_lists_all_students() {
    let router = app().await;
    for uri in ["/students?q=", "/students?q=+%E3%80%80+", "/students?"] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), ["5", "4", "3"], "{uri}");
    }
}

#[tokio::test]
async fn q_up_to_50_chars_is_accepted_and_longer_is_422() {
    let router = app().await;
    let (status, body) = list_students_at(
        &router,
        &format!("/students?q=+{}+", "a".repeat(50)),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "前後の空白は数えない: {body}");
    assert_eq!(body, json!({ "students": [] }));

    let (status, body) = list_students_at(
        &router,
        &format!("/students?q={}", "a".repeat(51)),
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
    assert_eq!(body["message"], "表示名の検索語は 50 文字まで");
}

#[tokio::test]
async fn sport_lists_only_students_of_the_sport() {
    let router = app().await;
    for (uri, expected) in [
        ("/students?sport=tennis", vec!["3"]),
        ("/students?sport=golf", vec!["4"]),
        ("/students?sport=swimming", vec!["5"]),
        ("/students?sport=soccer", vec![]),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), expected, "{uri}");
    }
}

#[tokio::test]
async fn not_booked_since_lists_students_without_bookings_from_the_jst_date() {
    let router = app().await;
    // 月額の生徒が最後に予約したのは 2026-10-01T12:00:00Z（日本時間の 10 月 1 日 21 時）
    for (uri, expected) in [
        ("/students?not_booked_since=2026-10-01", vec!["5", "4"]),
        ("/students?not_booked_since=2026-10-02", vec!["5", "4", "3"]),
        ("/students?not_booked_since=2026-09-01", vec!["5", "4"]),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), expected, "{uri}");
    }
    let (_, body) = list_students_at(
        &router,
        "/students?not_booked_since=2026-10-02",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(
        body["students"][2]["last_booked_at"], "2026-10-01T12:00:00Z",
        "絞り込んでも最後に予約した時刻を添える"
    );
}

#[tokio::test]
async fn q_sport_and_not_booked_since_are_combined() {
    let router = app().await;
    // 「なまえ」は生徒 3 人とも含む
    for (uri, expected) in [
        (
            "/students?q=%E3%81%AA%E3%81%BE%E3%81%88&sport=tennis&not_booked_since=2026-10-02",
            vec!["3"],
        ),
        (
            "/students?q=%E3%81%AA%E3%81%BE%E3%81%88&sport=tennis&not_booked_since=2026-10-01",
            vec![],
        ),
        (
            "/students?sport=golf&not_booked_since=2026-10-01",
            vec!["4"],
        ),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), expected, "{uri}");
    }
}

#[tokio::test]
async fn coach_id_lists_only_students_who_booked_the_coach() {
    let router = app().await;
    for (uri, expected) in [
        // 月額の生徒だけがコーチ 2 の枠を予約した
        (
            "/students?coach_id=00000000-0000-0000-0000-000000000002",
            vec!["3"],
        ),
        // ない ID・コーチでない ID（本部・生徒）は 404 にせず空の一覧
        (
            "/students?coach_id=00000000-0000-0000-0000-000000000009",
            vec![],
        ),
        (
            "/students?coach_id=00000000-0000-0000-0000-000000000001",
            vec![],
        ),
        (
            "/students?coach_id=00000000-0000-0000-0000-000000000003",
            vec![],
        ),
        // ほかの条件と組み合わせられる
        (
            "/students?coach_id=00000000-0000-0000-0000-000000000002&sport=tennis&not_booked_since=2026-10-02",
            vec!["3"],
        ),
        (
            "/students?coach_id=00000000-0000-0000-0000-000000000002&not_booked_since=2026-10-01",
            vec![],
        ),
        (
            "/students?coach_id=00000000-0000-0000-0000-000000000002&sport=golf",
            vec![],
        ),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), expected, "{uri}");
    }
    let (_, body) = list_students_at(
        &router,
        "/students?coach_id=00000000-0000-0000-0000-000000000002",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(body["students"][0]["contract_type"], "subscription");
    assert_eq!(
        body["students"][0]["last_booked_at"], "2026-10-01T12:00:00Z",
        "絞り込んでも契約と最後に予約した時刻を添える"
    );
}

#[tokio::test]
async fn has_unanswered_lists_only_students_with_an_unanswered_thread() {
    let router = app().await;
    for (uri, expected) in [
        // 月額の生徒だけが 24 時間ちょうど返信を待っている。ゴルフの生徒はコーチが返し、新しい生徒はスレッドがない
        ("/students?has_unanswered=true", vec!["3"]),
        // false は絞らない
        ("/students?has_unanswered=false", vec!["5", "4", "3"]),
        // ほかの条件と組み合わせられる
        ("/students?has_unanswered=true&sport=tennis", vec!["3"]),
        ("/students?has_unanswered=true&sport=golf", vec![]),
        (
            "/students?has_unanswered=true&q=%E3%81%AA%E3%81%BE%E3%81%883",
            vec!["3"],
        ),
        (
            "/students?has_unanswered=true&coach_id=00000000-0000-0000-0000-000000000002&not_booked_since=2026-10-02",
            vec!["3"],
        ),
        (
            "/students?has_unanswered=true&not_booked_since=2026-10-01",
            vec![],
        ),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), expected, "{uri}");
    }
    let (_, body) =
        list_students_at(&router, "/students?has_unanswered=true", Some(HEADQUARTERS)).await;
    assert_eq!(
        body["students"][0]["last_booked_at"], "2026-10-01T12:00:00Z",
        "絞り込んでも最後に予約した時刻を添える"
    );

    // 照会した時刻から数える（1 秒前なら 24 時間に満たない）
    let router = app_at(now() - Duration::from_secs(1)).await;
    let (status, body) =
        list_students_at(&router, "/students?has_unanswered=true", Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(listed_ids(&body), Vec::<&str>::new());
}

#[tokio::test]
async fn has_unanswered_that_is_not_a_boolean_is_422() {
    let router = app().await;
    for uri in [
        "/students?has_unanswered=yes",
        "/students?has_unanswered=1",
        "/students?has_unanswered=",
        "/students?has_unanswered=TRUE",
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{uri}: {body}");
        assert_eq!(body["code"], "validation_failed", "{uri}");
    }
}

#[tokio::test]
async fn has_pending_video_lists_only_students_with_a_pending_video() {
    let router = app().await;
    for (uri, expected) in [
        // 新しい生徒と月額の生徒に返却待ちの動画がある。ゴルフの生徒は返却済みだけ
        ("/students?has_pending_video=true", vec!["5", "3"]),
        // false は絞らない
        ("/students?has_pending_video=false", vec!["5", "4", "3"]),
        // ほかの条件と組み合わせられる
        ("/students?has_pending_video=true&sport=tennis", vec!["3"]),
        ("/students?has_pending_video=true&sport=golf", vec![]),
        (
            "/students?has_pending_video=true&q=%E3%81%AA%E3%81%BE%E3%81%885",
            vec!["5"],
        ),
        (
            "/students?has_pending_video=true&coach_id=00000000-0000-0000-0000-000000000002",
            vec!["3"],
        ),
        (
            "/students?has_pending_video=true&not_booked_since=2026-10-01",
            vec!["5"],
        ),
        (
            "/students?has_pending_video=true&has_unanswered=true",
            vec!["3"],
        ),
        (
            "/students?has_pending_video=true&has_unanswered=true&sport=swimming",
            vec![],
        ),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), expected, "{uri}");
    }
    let (_, body) = list_students_at(
        &router,
        "/students?has_pending_video=true",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(
        body["students"][1]["last_booked_at"], "2026-10-01T12:00:00Z",
        "絞り込んでも最後に予約した時刻を添える"
    );
}

#[tokio::test]
async fn has_pending_video_that_is_not_a_boolean_is_422() {
    let router = app().await;
    for uri in [
        "/students?has_pending_video=yes",
        "/students?has_pending_video=1",
        "/students?has_pending_video=",
        "/students?has_pending_video=TRUE",
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{uri}: {body}");
        assert_eq!(body["code"], "validation_failed", "{uri}");
    }
}

#[tokio::test]
async fn has_unhandled_low_rated_lists_only_students_with_an_unhandled_low_review() {
    let router = app().await;
    for (uri, expected) in [
        // 新しい生徒とゴルフの生徒が未対応の低い評価をつけた。月額の生徒は対応済み・★4・31 日前だけ
        ("/students?has_unhandled_low_rated=true", vec!["5", "4"]),
        // false は絞らない
        (
            "/students?has_unhandled_low_rated=false",
            vec!["5", "4", "3"],
        ),
        // ほかの条件と組み合わせられる
        (
            "/students?has_unhandled_low_rated=true&sport=golf",
            vec!["4"],
        ),
        (
            "/students?has_unhandled_low_rated=true&sport=tennis",
            vec![],
        ),
        (
            "/students?has_unhandled_low_rated=true&q=%E3%81%AA%E3%81%BE%E3%81%885",
            vec!["5"],
        ),
        (
            "/students?has_unhandled_low_rated=true&coach_id=00000000-0000-0000-0000-000000000002",
            vec![],
        ),
        (
            "/students?has_unhandled_low_rated=true&not_booked_since=2026-10-01",
            vec!["5", "4"],
        ),
        (
            "/students?has_unhandled_low_rated=true&has_pending_video=true",
            vec!["5"],
        ),
        (
            "/students?has_unhandled_low_rated=true&has_unanswered=true",
            vec![],
        ),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        assert_eq!(listed_ids(&body), expected, "{uri}");
    }
    let (_, body) = list_students_at(
        &router,
        "/students?has_unhandled_low_rated=true&sport=golf",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(
        body["students"][0]["contract_type"], "one_shot",
        "絞り込んでも受講権を添える"
    );

    // 期間は照会した時刻から数える（ゴルフの生徒の評価の 30 日後の 1 秒後には外れる）
    let later = app_at(at(5) + Duration::from_secs(30 * 24 * 3600 + 1)).await;
    let (status, body) = list_students_at(
        &later,
        "/students?has_unhandled_low_rated=true",
        Some(HEADQUARTERS),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(listed_ids(&body), vec!["5"]);
}

#[tokio::test]
async fn has_unhandled_low_rated_that_is_not_a_boolean_is_422() {
    let router = app().await;
    for uri in [
        "/students?has_unhandled_low_rated=yes",
        "/students?has_unhandled_low_rated=1",
        "/students?has_unhandled_low_rated=",
        "/students?has_unhandled_low_rated=TRUE",
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{uri}: {body}");
        assert_eq!(body["code"], "validation_failed", "{uri}");
    }
}

#[tokio::test]
async fn unknown_sport_or_malformed_date_is_422() {
    let router = app().await;
    for (uri, message) in [
        ("/students?sport=cricket", "未知のスポーツ: cricket"),
        ("/students?sport=", "未知のスポーツ: "),
        (
            "/students?not_booked_since=2026-10-1",
            "not_booked_since は YYYY-MM-DD の日付でない",
        ),
        (
            "/students?not_booked_since=2026-02-30",
            "not_booked_since は YYYY-MM-DD の日付でない",
        ),
        (
            "/students?not_booked_since=2026-10-01T00:00:00Z",
            "not_booked_since は YYYY-MM-DD の日付でない",
        ),
        ("/students?coach_id=coach-2", "coach_id が UUID でない"),
        ("/students?coach_id=", "coach_id が UUID でない"),
    ] {
        let (status, body) = list_students_at(&router, uri, Some(HEADQUARTERS)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{uri}: {body}");
        assert_eq!(body["code"], "validation_failed", "{uri}");
        assert_eq!(body["message"], message, "{uri}");
    }
}

#[tokio::test]
async fn no_students_is_an_empty_list() {
    let accounts = InMemoryAccountRepository::default();
    accounts
        .insert(account(COACH, Role::Coach, &[Sport::Tennis], 0))
        .await
        .unwrap();
    let router = router(
        accounts,
        InMemoryEntitlementRepository::default(),
        InMemoryBookingRepository::new(InMemorySlotRepository::default()),
        InMemoryMessageThreadRepository::default(),
        InMemoryVideoRepository::default(),
        InMemoryLessonReviewRepository::default(),
        now(),
    );
    let (status, body) = list_students(&router, Some(HEADQUARTERS)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "students": [] }));
}

#[tokio::test]
async fn coach_and_student_are_403_and_no_token_is_401() {
    let router = app().await;
    for token in ["coach:2", "student:3"] {
        let (status, body) = list_students(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
        // 条件を付けても（不正な値でも）役割の確認が先
        let (status, body) = list_students_at(
            &router,
            "/students?sport=cricket&not_booked_since=x",
            Some(token),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
    }

    let (status, body) = list_students(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}

/// 予約の保存先が障害を返す偽物。
struct BrokenBookings;

impl application::identity::StudentLastBookingLookup for BrokenBookings {
    async fn find_last_booked_at(
        &self,
        _student_ids: &std::collections::BTreeSet<AccountId>,
    ) -> Result<std::collections::BTreeMap<AccountId, SystemTime>, application::Unavailable> {
        Err(application::Unavailable::new(
            "connection to postgres://rally:secret@db.internal:5432 refused",
        ))
    }
}

#[tokio::test]
async fn lookup_failure_is_500_internal_without_the_reason() {
    let accounts = InMemoryAccountRepository::default();
    accounts
        .insert(account(NEW_STUDENT, Role::Student, &[Sport::Swimming], 0))
        .await
        .unwrap();
    let router = interface::students_router(
        ListStudents::new(
            accounts,
            EntitlementStudentContractLookup::new(InMemoryEntitlementRepository::default()),
            BrokenBookings,
            SchedulingStudentCoachBookingLookup::new(InMemoryBookingRepository::new(
                InMemorySlotRepository::default(),
            )),
            SchedulingStudentRecentBookingCountLookup::new(InMemoryBookingRepository::new(
                InMemorySlotRepository::default(),
            )),
            MessagingStudentUnansweredMessageThreadCountLookup::new(
                InMemoryMessageThreadRepository::default(),
            ),
            AsyncLessonStudentVideoActivityLookup::new(InMemoryVideoRepository::default()),
            ReviewStudentLowRatedReviewCountLookup::new(InMemoryLessonReviewRepository::default()),
            FixedClock(now()),
        ),
        FakeTokens,
    );
    // 契約は 500 の応答を書かないので、形は検査しない
    let response = router
        .oneshot(
            Request::get("/students")
                .header(header::AUTHORIZATION, format!("Bearer {HEADQUARTERS}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["code"], "internal");
    assert!(!body.to_string().contains("secret"), "{body}");
}
