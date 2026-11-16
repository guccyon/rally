//! `/business-kpis` の統合テスト。admin-web の `GET /business-kpis`（`getBusinessKpis`）のレスポンスの形を OpenAPI の
//! スキーマで検査し、本部のトークンで specs/analytics/kpi.yaml の 6 指標が kpi.yaml の順に今の期間と前の期間の値で返ること
//! （式は kpi.yaml の正本をそのまま読んで評価する）・母集団がない／観測し終えていない／群の下限を満たさない期間は null と
//! 理由になること・1 つのイベントが読めなくても 200 でその指標だけ unavailable になること・コーチと生徒は 403、トークンが
//! なければ 401 になることを確かめる。

use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use application::{
    analytics::{GetBusinessKpis, KpiEvent},
    identity::{AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier},
    in_memory::{FixedClock, FixedKpiEvents, FixedKpiSpec},
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use serde_json::Value;
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

const HEADQUARTERS: &str = "headquarters:5";
const COACH: &str = "coach:9";
const STUDENT: &str = "student:1";

/// 集計した時刻 2026-11-16T00:00:00Z。
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_787_200)
}

const HOUR: Duration = Duration::from_secs(3600);
const DAY: Duration = Duration::from_secs(24 * 3600);

fn days_ago(days: u32) -> SystemTime {
    now() - DAY * days
}

/// 式の正本（このテストのために写さない）。
fn kpi_yaml() -> FixedKpiSpec {
    let path = format!(
        "{}/../../../../specs/analytics/kpi.yaml",
        env!("CARGO_MANIFEST_DIR")
    );
    FixedKpiSpec(Arc::from(
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")),
    ))
}

fn app(events: FixedKpiEvents) -> Router {
    interface::business_kpis_router(
        GetBusinessKpis::new(kpi_yaml(), events, FixedClock(now())),
        FakeTokens,
    )
}

fn lesson(student: &str, coach: &str, first: bool, at: SystemTime) -> KpiEvent {
    KpiEvent::new("lesson.completed")
        .with("student_id", student)
        .with("coach_id", coach)
        .with("lesson_id", format!("{student}-{at:?}"))
        .with("lesson_type", "live_lesson")
        .with("is_first_lesson", first)
        .with("occurred_at", at)
}

/// コーチ `coach` の初回のレッスンを `count` 人が `first_at` から 1 時間おきに受け、先頭の `retained` 人は翌日に月額を
/// 始め、初回から 40 日目にもレッスンを完了する（継続）。
fn first_lessons(coach: &str, first_at: SystemTime, count: u32, retained: u32) -> Vec<KpiEvent> {
    let mut events = Vec::new();
    for i in 0..count {
        let student = format!("{coach}-student-{i}");
        let at = first_at + HOUR * i;
        events.push(lesson(&student, coach, true, at));
        if i < retained {
            events.push(
                KpiEvent::new("subscription.started")
                    .with("student_id", student.as_str())
                    .with("subscription_id", format!("{student}-subscription"))
                    .with("sport", "tennis")
                    .with("occurred_at", at + DAY),
            );
            events.push(lesson(&student, coach, false, at + DAY * 40));
        }
    }
    events
}

fn slot(id: &str, starts_at: SystemTime) -> KpiEvent {
    KpiEvent::new("slot.published")
        .with("coach_id", "coach-a")
        .with("slot_id", id)
        .with("starts_at", starts_at)
        .with("occurred_at", starts_at - DAY * 7)
}

fn booking(name: &str, slot: &str, at: SystemTime) -> KpiEvent {
    KpiEvent::new(name)
        .with("student_id", format!("student-of-{slot}"))
        .with("coach_id", "coach-a")
        .with("booking_id", format!("booking-of-{slot}"))
        .with("slot_id", slot)
        .with("occurred_at", at)
}

fn call(at: SystemTime, loss: f64, jitter: f64, round_trip: f64, disconnects: f64) -> KpiEvent {
    KpiEvent::new("call.ended")
        .with("session_id", format!("session-{at:?}"))
        .with("packet_loss_rate", loss)
        .with("jitter_ms", jitter)
        .with("round_trip_ms", round_trip)
        .with("disconnect_count", disconnects)
        .with("occurred_at", at)
}

fn video(id: &str, submitted_at: SystemTime, returned_after: Option<Duration>) -> Vec<KpiEvent> {
    let mut events = vec![
        KpiEvent::new("video.submitted")
            .with("video_id", id)
            .with("occurred_at", submitted_at),
    ];
    if let Some(after) = returned_after {
        events.push(
            KpiEvent::new("video_review.returned")
                .with("video_id", id)
                .with("submitted_at", submitted_at)
                .with("occurred_at", submitted_at + after),
        );
    }
    events
}

fn trial(student: &str, booked_at: SystemTime, first_lesson_at: SystemTime) -> Vec<KpiEvent> {
    vec![
        KpiEvent::new("booking.created")
            .with("student_id", student)
            .with("booking_id", format!("trial-{student}"))
            .with("is_trial", true)
            .with("occurred_at", booked_at),
        lesson(student, "coach-t", true, first_lesson_at),
    ]
}

/// 6 指標のどれにも、今の期間（継続率と分散は 90〜60 日前、ほかは直近 30 日）と前の期間に母集団がある（通話品質だけ
/// 前の期間に通話がない）。
fn seeded() -> Vec<KpiEvent> {
    let mut events = Vec::new();
    // 継続率と分散: 今の期間はコーチ A が 5 人中 4 人、B が 5 人中 1 人が継続。前の期間は C の 5 人全員
    events.extend(first_lessons("coach-a", days_ago(80), 5, 4));
    events.extend(first_lessons("coach-b", days_ago(70), 5, 1));
    events.extend(first_lessons("coach-c", days_ago(100), 5, 5));
    // 充足率: 今の期間は残った予約 1・キャンセルされた予約 1・取り下げ 1・まだ始まらない枠 1。前の期間は残った予約 1
    events.push(slot("slot-1", days_ago(10)));
    events.push(booking("booking.confirmed", "slot-1", days_ago(12)));
    events.push(slot("slot-2", days_ago(5)));
    events.push(booking("booking.confirmed", "slot-2", days_ago(6)));
    events.push(booking("booking.cancelled", "slot-2", days_ago(5) - HOUR));
    events.push(slot("slot-3", days_ago(3)));
    events.push(
        KpiEvent::new("slot.withdrawn")
            .with("slot_id", "slot-3")
            .with("occurred_at", days_ago(4)),
    );
    events.push(slot("slot-4", now() + DAY));
    events.push(slot("slot-5", days_ago(40)));
    events.push(booking("booking.confirmed", "slot-5", days_ago(41)));
    // 通話品質: 今の期間に 87 点（損失 1%・ゆらぎ 40ms・往復 200ms）と 100 点
    events.push(call(days_ago(1), 1.0, 40.0, 200.0, 0.0));
    events.push(call(days_ago(2), 0.0, 0.0, 0.0, 0.0));
    // 返却リードタイム: 今の期間に 10 時間・未返却（集計時刻まで 24 時間）・2 時間。前の期間に 5 時間
    events.extend(video("video-1", days_ago(2), Some(HOUR * 10)));
    events.extend(video("video-2", days_ago(1), None));
    events.extend(video("video-3", days_ago(3), Some(HOUR * 2)));
    events.extend(video("video-4", days_ago(40), Some(HOUR * 5)));
    // 初回レッスンまでの日数: 今の期間に 3 日、前の期間に 2 日
    events.extend(trial("trial-1", days_ago(10), days_ago(7)));
    events.extend(trial("trial-2", days_ago(50), days_ago(48)));
    events
}

async fn get_kpis(router: &Router, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/business-kpis");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("admin-web").assert_response("get", "/business-kpis", status, &body);
    (status, body)
}

fn kpi<'a>(body: &'a Value, id: &str) -> &'a Value {
    body["kpis"]
        .as_array()
        .unwrap()
        .iter()
        .find(|kpi| kpi["id"] == id)
        .unwrap_or_else(|| panic!("{id} がない: {body}"))
}

fn value(body: &Value, id: &str, period: &str) -> f64 {
    let value = &kpi(body, id)[period];
    assert_eq!(value["reason"], Value::Null, "{id} の {period}: {value}");
    value["value"].as_f64().unwrap()
}

fn reason<'a>(body: &'a Value, id: &str, period: &str) -> &'a Value {
    let value = &kpi(body, id)[period];
    assert_eq!(value["value"], Value::Null, "{id} の {period}: {value}");
    &value["reason"]
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{what}: {actual} ≠ {expected}"
    );
}

#[tokio::test]
async fn headquarters_reads_six_kpis_in_kpi_yaml_order_for_this_and_previous_period() {
    let (status, body) = get_kpis(&app(FixedKpiEvents::new(seeded())), Some(HEADQUARTERS)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["computed_at"], "2026-11-16T00:00:00Z");
    let ids: Vec<&str> = body["kpis"]
        .as_array()
        .unwrap()
        .iter()
        .map(|kpi| kpi["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "first_lesson_retention_1m",
            "coach_retention_variance",
            "slot_fill_rate",
            "call_quality_score",
            "video_review_lead_time",
            "days_to_first_lesson",
        ],
        "kpi.yaml の順"
    );

    let north_star = kpi(&body, "first_lesson_retention_1m");
    assert_eq!(north_star["name"], "初回レッスン後の 1 ヶ月継続率");
    assert_eq!(north_star["north_star"], true);
    assert_eq!(north_star["direction"], "up_is_good");
    assert_eq!(north_star["unit"], "ratio");
    assert_eq!(north_star["population_label"], "生徒");
    assert_eq!(
        north_star["current"]["window_start"], "2026-08-18T00:00:00Z",
        "継続率は 60 日目を観測し終えた生徒だけ（集計時刻の 60 日前に終わる 30 日）"
    );
    assert_eq!(north_star["current"]["window_end"], "2026-09-17T00:00:00Z");
    assert_eq!(
        north_star["previous"]["window_start"],
        "2026-07-19T00:00:00Z"
    );
    assert_eq!(north_star["previous"]["window_end"], "2026-08-18T00:00:00Z");
    assert_eq!(
        body["kpis"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|kpi| kpi["north_star"] == true)
            .count(),
        1
    );

    let fill = kpi(&body, "slot_fill_rate");
    assert_eq!(fill["direction"], "up_is_good");
    assert_eq!(fill["population_label"], "枠");
    assert_eq!(fill["current"]["window_start"], "2026-10-17T00:00:00Z");
    assert_eq!(fill["current"]["window_end"], "2026-11-16T00:00:00Z");
    assert_eq!(fill["previous"]["window_start"], "2026-09-17T00:00:00Z");
    assert_eq!(
        kpi(&body, "video_review_lead_time")["direction"],
        "down_is_good"
    );

    assert_close(
        value(&body, "first_lesson_retention_1m", "current"),
        0.5,
        "今の継続率",
    );
    assert_close(
        value(&body, "first_lesson_retention_1m", "previous"),
        1.0,
        "前の継続率",
    );
    assert_close(
        value(&body, "coach_retention_variance", "current"),
        0.09,
        "コーチ A 0.8・B 0.2 の母分散",
    );
    assert_close(
        value(&body, "coach_retention_variance", "previous"),
        0.0,
        "群が 1 つ",
    );
    assert_close(
        value(&body, "slot_fill_rate", "current"),
        0.5,
        "キャンセルされた枠は埋まらず、取り下げとまだ始まらない枠は分母に入らない",
    );
    assert_close(
        value(&body, "slot_fill_rate", "previous"),
        1.0,
        "前の充足率",
    );
    assert_close(
        value(&body, "call_quality_score", "current"),
        93.5,
        "87 点と 100 点",
    );
    assert_close(
        value(&body, "video_review_lead_time", "current"),
        10.0,
        "2・10・24（未返却は集計時刻まで）の中央値",
    );
    assert_close(
        value(&body, "video_review_lead_time", "previous"),
        5.0,
        "前の返却",
    );
    assert_close(
        value(&body, "days_to_first_lesson", "current"),
        3.0,
        "今の日数",
    );
    assert_close(
        value(&body, "days_to_first_lesson", "previous"),
        2.0,
        "前の日数",
    );

    let no_calls = reason(&body, "call_quality_score", "previous");
    assert_eq!(no_calls["code"], "no_population", "前の期間に通話がない");
    assert_eq!(
        no_calls["message"],
        "この期間に数える通話がないため、出せません。"
    );
}

#[tokio::test]
async fn without_any_event_all_six_kpis_are_null_with_no_population() {
    let (status, body) = get_kpis(&app(FixedKpiEvents::default()), Some(HEADQUARTERS)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let kpis = body["kpis"].as_array().unwrap();
    assert_eq!(kpis.len(), 6, "値が出せない指標も省かない");
    for kpi in kpis {
        for period in ["current", "previous"] {
            assert_eq!(kpi[period]["value"], Value::Null, "{kpi}");
            assert_eq!(kpi[period]["reason"]["code"], "no_population", "{kpi}");
        }
    }
}

#[tokio::test]
async fn first_lessons_not_yet_observed_for_60_days_are_not_observable_yet() {
    // 初回のレッスンが 10 日前の生徒しかいない（60 日目まで見終えていない）
    let events = first_lessons("coach-a", days_ago(10), 5, 0);

    let (status, body) = get_kpis(&app(FixedKpiEvents::new(events)), Some(HEADQUARTERS)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    for id in ["first_lesson_retention_1m", "coach_retention_variance"] {
        for period in ["current", "previous"] {
            assert_eq!(
                reason(&body, id, period)["code"],
                "not_observable_yet",
                "{id} {period}"
            );
        }
    }
    assert_eq!(
        reason(&body, "first_lesson_retention_1m", "current")["message"],
        "60 日目まで見終えた生徒がまだいないため、出せません。"
    );
}

#[tokio::test]
async fn coaches_below_five_first_lessons_give_below_group_minimum() {
    // 観測し終えた初回のレッスンが、コーチ A に 4 件だけ
    let events = first_lessons("coach-a", days_ago(80), 4, 2);

    let (status, body) = get_kpis(&app(FixedKpiEvents::new(events)), Some(HEADQUARTERS)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_close(
        value(&body, "first_lesson_retention_1m", "current"),
        0.5,
        "継続率は出る",
    );
    let below = reason(&body, "coach_retention_variance", "current");
    assert_eq!(below["code"], "below_group_minimum");
    assert_eq!(
        below["message"],
        "初回レッスンが 5 件以上のコーチがいないため、出せません。"
    );
}

#[tokio::test]
async fn one_unreadable_event_makes_only_its_kpi_unavailable_with_200() {
    let events = FixedKpiEvents::new(seeded()).failing("call.ended");

    let (status, body) = get_kpis(&app(events), Some(HEADQUARTERS)).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    for period in ["current", "previous"] {
        let failed = reason(&body, "call_quality_score", period);
        assert_eq!(failed["code"], "unavailable");
        assert_eq!(
            failed["message"],
            "集計に失敗したため、出せません。時間をおいて開き直してください。"
        );
    }
    assert_close(
        value(&body, "slot_fill_rate", "current"),
        0.5,
        "ほかの指標はそのまま",
    );
    assert_close(
        value(&body, "first_lesson_retention_1m", "current"),
        0.5,
        "ほかの指標はそのまま",
    );
}

#[tokio::test]
async fn coaches_and_students_are_forbidden_and_no_token_is_unauthorized() {
    let router = app(FixedKpiEvents::new(seeded()));

    for token in [COACH, STUDENT] {
        let (status, body) = get_kpis(&router, Some(token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = get_kpis(&router, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
}
