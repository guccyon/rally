//! `GET /coaches/{coach_id}/reviews`（`listCoachReviews`）と `GET /coaches/{coach_id}/review-summary`
//! （`getCoachReviewSummary`）の統合テスト。どちらも admin-web と student-app の契約にある同じ操作なので、レスポンスの形を
//! 両方の OpenAPI のスキーマで検査する。本部と生徒はどのコーチの分も、コーチは自分の分だけを読めること（他のコーチの分は
//! 403）と、送った時刻の新しい順に並ぶこと、評価がなければ 0 件になることを確かめる。

use std::time::{Duration, SystemTime};

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    entitlement::{EntitlementQuotaGate, EntitlementReviewGate, GetEntitlement, StartSubscription},
    identity::{
        AccessToken, InvalidToken, RegisterAccount, TokenClaims, TokenIssuer, TokenVerifier,
    },
    in_memory::{
        FixedAiFeedbackGenerator, FixedClock, FixedVideoCallProvider, FixedVideoStorage,
        InMemoryAccountRepository, InMemoryBookingRepository, InMemoryEntitlementRepository,
        InMemoryLessonCompletionRepository, InMemoryLessonReviewRepository,
        InMemoryLiveLessonSessionRepository, InMemorySlotRepository, InMemoryVideoRepository,
        RecordingAnalyticsSink, RecordingDomainEventPublisher,
    },
    lesson::RecordLessonCompletion,
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    review::{GetCoachReviewSummary, ListCoachReviews, SubmitLessonReview},
    scheduling::{
        CancelBooking, ConfirmBooking, CreateBooking, GetSlotSupply, PublishSlot,
        SchedulingBookingLookup, SearchSlots,
    },
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    identity::{AccountId, Role},
    lesson::LessonId,
    review::{LessonReview, LessonReviewComment, LessonReviewId, Rating},
};
use serde_json::{Value, json};
use tower::ServiceExt;

mod support;

/// 2026-10-26T00:00:00Z（現在時刻）
const NOW: Duration = Duration::from_secs(1_792_972_800);
const HOUR: Duration = Duration::from_secs(3600);

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
/// 評価を受けたコーチ（ID 42 = 0x2a）と、そのトークン
const COACH_ID: &str = "00000000-0000-0000-0000-00000000002a";
const COACH: &str = "coach:42";
const OTHER_COACH: &str = "coach:43";
/// 評価したどの生徒（ID 101〜104）でもない生徒
const STUDENT: &str = "student:3";
/// 評価の対象のコーチ 42 に評価を付けた生徒（review 2 の ID 102）
const REVIEWER_STUDENT: &str = "student:102";

fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + NOW
}

fn review(id: u128, coach: u128, rating: i64, comment: &str, hours_ago: u32) -> LessonReview {
    LessonReview::restore(
        LessonReviewId::from_u128(id),
        LessonId::from_u128(10 + id),
        AccountId::from_u128(100 + id),
        AccountId::from_u128(coach),
        Rating::new(rating).unwrap(),
        LessonReviewComment::new(comment).unwrap(),
        now() - HOUR * hours_ago,
    )
}

/// コーチ 42 への評価 3 件（保存した順は新しい順でない）と、コーチ 43 への評価 1 件。
fn reviews() -> InMemoryLessonReviewRepository {
    InMemoryLessonReviewRepository::with_reviews([
        review(1, 42, 3, "ふつう", 48),
        review(2, 42, 5, "とてもよい", 1),
        review(3, 43, 1, "別のコーチ", 2),
        review(4, 42, 4, "よい", 24),
    ])
}

fn app(reviews: InMemoryLessonReviewRepository) -> Router {
    let analytics = RecordingAnalyticsSink::default();
    let clock = FixedClock(now());
    let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::default());
    let slots = bookings.slots().clone();
    interface::router_with(
        RegisterAccount::new(
            InMemoryAccountRepository::default(),
            analytics.clone(),
            clock,
        ),
        application::identity::ListCoaches::new(InMemoryAccountRepository::default()),
        application::identity::GetAccount::new(InMemoryAccountRepository::default()),
        application::identity::UpdateCoachProfile::new(InMemoryAccountRepository::default()),
        PublishSlot::new(slots.clone(), analytics.clone(), clock),
        SearchSlots::new(
            slots,
            application::identity::IdentityCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
            clock,
        ),
        CreateBooking::new(bookings.clone(), analytics.clone(), clock),
        ConfirmBooking::new(
            bookings.clone(),
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::default()),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        CancelBooking::new(
            bookings.clone(),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetSlotSupply::new(bookings, clock),
        application::scheduling::ListMyBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::identity::IdentityCoachIntroductionLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        application::scheduling::ListCoachBookings::new(
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
            application::identity::IdentityStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        application::scheduling::ListCoachSlots::new(
            InMemorySlotRepository::default(),
            InMemoryBookingRepository::new(InMemorySlotRepository::default()),
        ),
        StartSubscription::new(
            InMemoryEntitlementRepository::default(),
            analytics.clone(),
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        GetEntitlement::new(InMemoryEntitlementRepository::default(), clock),
        StartLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            SchedulingBookingLookup::new(InMemoryBookingRepository::new(
                InMemorySlotRepository::default(),
            )),
            FixedVideoCallProvider::default(),
            analytics.clone(),
            clock,
        ),
        EndLiveLessonSession::new(
            InMemoryLiveLessonSessionRepository::default(),
            FixedVideoCallProvider::default(),
            analytics.clone(),
            clock,
        ),
        SubmitVideo::new(
            InMemoryVideoRepository::default(),
            FixedVideoStorage,
            FixedAiFeedbackGenerator,
            analytics.clone(),
            clock,
        ),
        GetVideoReview::new(InMemoryVideoRepository::default()),
        ReturnVideoReview::new(
            InMemoryVideoRepository::default(),
            EntitlementReviewGate,
            analytics.clone(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock,
        ),
        application::async_lesson::ListCoachVideos::new(
            InMemoryVideoRepository::default(),
            application::identity::IdentityStudentDisplayNameLookup::new(
                InMemoryAccountRepository::default(),
            ),
        ),
        RecordLessonCompletion::new(
            InMemoryLiveLessonSessionRepository::default(),
            InMemoryVideoRepository::default(),
            InMemoryLessonCompletionRepository::default(),
            analytics,
            RecordingDomainEventPublisher::default(),
            clock,
        ),
        application::messaging::ListMessageThreads::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
        ),
        application::messaging::ListMessages::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
        ),
        application::messaging::SendMessage::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            application::entitlement::EntitlementChatGate::new(
                application::in_memory::InMemoryEntitlementRepository::default(),
            ),
            RecordingAnalyticsSink::default(),
            application::in_memory::RecordingDomainEventPublisher::default(),
            clock,
        ),
        application::messaging::MarkMessageThreadRead::new(
            application::in_memory::InMemoryMessageThreadRepository::default(),
            clock,
        ),
        SubmitLessonReview::new(
            application::lesson::LessonCompletionLookup::new(
                application::in_memory::InMemoryLessonCompletionRepository::default(),
            ),
            reviews.clone(),
            RecordingAnalyticsSink::default(),
            clock,
        ),
        GetCoachReviewSummary::new(reviews.clone()),
        ListCoachReviews::new(reviews),
        application::notification::GetNotificationSettings::new(
            application::in_memory::InMemoryNotificationSettingsRepository::default(),
        ),
        application::notification::UpdateNotificationSettings::new(
            application::in_memory::InMemoryNotificationSettingsRepository::default(),
        ),
        FakeTokens,
    )
}

/// `GET <path>` を送り、レスポンスの形を admin-web と student-app の両方の契約で検査する。
async fn get(
    router: &Router,
    path: &str,
    template: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    let mut request = Request::get(path);
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
    for app in ["admin-web", "student-app"] {
        support::Contract::load(app).assert_response("get", template, status, &body);
    }
    (status, body)
}

async fn list_coach_reviews(
    router: &Router,
    coach_id: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    get(
        router,
        &format!("/coaches/{coach_id}/reviews"),
        "/coaches/{coach_id}/reviews",
        token,
    )
    .await
}

async fn get_coach_review_summary(
    router: &Router,
    coach_id: &str,
    token: Option<&str>,
) -> (StatusCode, Value) {
    get(
        router,
        &format!("/coaches/{coach_id}/review-summary"),
        "/coaches/{coach_id}/review-summary",
        token,
    )
    .await
}

#[tokio::test]
async fn headquarters_the_coach_and_students_read_the_reviews_newest_first() {
    let router = app(reviews());
    for token in [HEADQUARTERS, COACH, STUDENT, REVIEWER_STUDENT] {
        let (status, body) = list_coach_reviews(&router, COACH_ID, Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{token}: {body}");
        assert_eq!(
            body,
            json!({
                "coach_id": COACH_ID,
                // 送った時刻の新しい順。他のコーチへの評価と、評価した生徒（student_id）は含まない
                "reviews": [
                    {
                        "review_id": "00000000-0000-0000-0000-000000000002",
                        "lesson_id": "00000000-0000-0000-0000-00000000000c",
                        "rating": 5,
                        "comment": "とてもよい",
                        "submitted_at": "2026-10-25T23:00:00Z",
                    },
                    {
                        "review_id": "00000000-0000-0000-0000-000000000004",
                        "lesson_id": "00000000-0000-0000-0000-00000000000e",
                        "rating": 4,
                        "comment": "よい",
                        "submitted_at": "2026-10-25T00:00:00Z",
                    },
                    {
                        "review_id": "00000000-0000-0000-0000-000000000001",
                        "lesson_id": "00000000-0000-0000-0000-00000000000b",
                        "rating": 3,
                        "comment": "ふつう",
                        "submitted_at": "2026-10-24T00:00:00Z",
                    },
                ],
            }),
            "{token}"
        );
    }
}

#[tokio::test]
async fn a_student_reads_the_reviews_of_any_coach() {
    let router = app(reviews());
    let other_coach_id = "00000000-0000-0000-0000-00000000002b";
    let (status, body) = list_coach_reviews(&router, other_coach_id, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "coach_id": other_coach_id,
            "reviews": [{
                "review_id": "00000000-0000-0000-0000-000000000003",
                "lesson_id": "00000000-0000-0000-0000-00000000000d",
                "rating": 1,
                "comment": "別のコーチ",
                "submitted_at": "2026-10-25T22:00:00Z",
            }],
        })
    );

    let (status, body) = get_coach_review_summary(&router, other_coach_id, Some(STUDENT)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "coach_id": other_coach_id, "review_count": 1, "average_rating": 1.0 })
    );
}

#[tokio::test]
async fn a_coach_without_reviews_has_an_empty_list() {
    let router = app(reviews());
    let coach_id = "00000000-0000-0000-0000-000000000063";
    for token in [HEADQUARTERS, STUDENT] {
        let (status, body) = list_coach_reviews(&router, coach_id, Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{token}: {body}");
        assert_eq!(body, json!({ "coach_id": coach_id, "reviews": [] }));
    }
}

#[tokio::test]
async fn headquarters_the_coach_and_students_read_the_summary() {
    let router = app(reviews());
    for token in [HEADQUARTERS, COACH, STUDENT] {
        let (status, body) = get_coach_review_summary(&router, COACH_ID, Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{token}: {body}");
        // (3 + 5 + 4) / 3 = 4.0。他のコーチへの評価は数えない
        assert_eq!(
            body,
            json!({ "coach_id": COACH_ID, "review_count": 3, "average_rating": 4.0 }),
            "{token}"
        );
    }
}

#[tokio::test]
async fn a_coach_without_reviews_has_no_average() {
    let router = app(reviews());
    let coach_id = "00000000-0000-0000-0000-000000000063";
    for token in [HEADQUARTERS, STUDENT] {
        let (status, body) = get_coach_review_summary(&router, coach_id, Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{token}: {body}");
        assert_eq!(
            body,
            json!({ "coach_id": coach_id, "review_count": 0, "average_rating": null })
        );
    }
}

#[tokio::test]
async fn other_coaches_may_not_read_the_reviews() {
    let router = app(reviews());
    let (status, body) = list_coach_reviews(&router, COACH_ID, Some(OTHER_COACH)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "forbidden");
    let (status, body) = get_coach_review_summary(&router, COACH_ID, Some(OTHER_COACH)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "forbidden");

    let (status, body) = list_coach_reviews(&router, COACH_ID, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
    let (status, body) = get_coach_review_summary(&router, COACH_ID, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn a_coach_id_that_is_not_a_uuid_is_not_found() {
    let router = app(reviews());
    for token in [HEADQUARTERS, STUDENT] {
        let (status, body) = list_coach_reviews(&router, "not-a-uuid", Some(token)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token}: {body}");
        assert_eq!(body["code"], "not_found");
        let (status, body) = get_coach_review_summary(&router, "not-a-uuid", Some(token)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{token}: {body}");
        assert_eq!(body["code"], "not_found");
    }
}
