//! `GET /students`（admin-web の `listStudents`）の統合テスト。本部には生徒の一覧を、受講権の契約種別と契約プラン・
//! 最後に予約した時刻とともに返すことを OpenAPI のスキーマで検査する。コーチ・生徒は 403、トークンがなければ 401。
//! この操作はパスパラメータもクエリも受けないので、404（見つからない）と 422（規則違反）の応答はない。

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
    let mut request = Request::get("/students");
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
