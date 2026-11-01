//! `GET /students`（admin-web の `listStudents`）の統合テスト。本部には生徒の一覧を、受講権の契約種別と契約プラン・
//! 最後に予約した時刻とともに返すことを OpenAPI のスキーマで検査する。コーチ・生徒は 403、トークンがなければ 401。
//! 検索語 `q` を付ければ表示名に部分一致する生徒だけを返し、長すぎる `q` は 422。`sport`・`not_booked_since` を付ければ
//! そのスポーツの・その日（日本時間）以降に予約していない生徒だけを返し、値が不正なら 422。この操作はパスパラメータを受けないので、
//! 404（見つからない）の応答はない。

use std::time::{Duration, SystemTime};

use application::{
    entitlement::EntitlementStudentContractLookup,
    identity::{
        AccessToken, AccountRepository, InvalidToken, ListStudents, TokenClaims, TokenIssuer,
        TokenVerifier,
    },
    in_memory::{
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemorySlotRepository,
    },
    scheduling::{BookingRepository, SchedulingStudentLastBookingLookup},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    entitlement::{
        Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan, YearMonth,
    },
    identity::{Account, AccountId, Email, Role},
    profile::DisplayName,
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

/// 本部・コーチと、生徒 3 人（登録は月額の生徒 → ゴルフの生徒 → 新しい生徒の順）。月額の生徒は 2 回予約した。
async fn app() -> Router {
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
    router(accounts, entitlements, bookings)
}

fn router(
    accounts: InMemoryAccountRepository,
    entitlements: InMemoryEntitlementRepository,
    bookings: InMemoryBookingRepository,
) -> Router {
    interface::students_router(
        ListStudents::new(
            accounts,
            EntitlementStudentContractLookup::new(entitlements),
            SchedulingStudentLastBookingLookup::new(bookings),
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
                    "registered_at": "2026-10-01T03:00:00Z",
                },
                {
                    "student_id": "00000000-0000-0000-0000-000000000004",
                    "display_name": "なまえ4",
                    "sport": "golf",
                    "contract_type": "one_shot",
                    "plan": null,
                    "last_booked_at": null,
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
                    "registered_at": "2026-10-01T01:00:00Z",
                },
            ],
        }),
        "生徒だけを載せ、メールアドレスは載せない"
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
