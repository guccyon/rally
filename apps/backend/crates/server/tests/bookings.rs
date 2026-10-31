//! `/bookings` を組み立て済みのルーター（`server::router()`）で通す。コーチが枠を公開し、`POST /accounts` で
//! 登録した生徒が体験予約して確定するまで（一周シナリオの入口）と、確定した予約のキャンセル。境界間イベントは
//! infra のモック（標準出力に書くだけ）。契約の形・拒否の細かい検査は interface の `tests/bookings.rs`。
//! 自分の予約の一覧（`GET /bookings/me`）は本人の分だけを返し、生徒以外は 403。
//! コーチの自分の予約の一覧（`GET /coaches/me/bookings`）は本人の枠への予約だけを生徒の表示名と一緒に返し、コーチ以外は 403。
//! 本部が見る予約の一覧（`GET /bookings`）は同じパスの `POST /bookings`（生徒）と並んで、すべてのコーチの枠への予約を
//! 日付（日本時間）で絞って返し、本部以外は 403。

use application::identity::{TokenClaims, TokenIssuer};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::identity::{AccountId, Role};
use infra::MockTokenService;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn post_json(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut request = Request::post(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    request.body(Body::from(body.to_string())).unwrap()
}

async fn register(router: &Router, email: &str) -> String {
    let (status, body) = send(
        router,
        post_json(
            "/accounts",
            None,
            json!({ "email": email, "display_name": "たろう", "sport": "tennis" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn student_books_a_trial_and_confirms_it() {
    let router = server::router();
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            Some(&coach),
            json!({
                "sport": "tennis",
                "starts_at": "2099-01-01T10:00:00Z",
                "ends_at": "2099-01-01T11:00:00Z",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let student = register(&router, "taro@example.com").await;
    let other = register(&router, "hanako@example.com").await;
    let request = json!({ "slot_id": slot["slot_id"], "is_trial": true });

    // コーチは予約できない
    let (status, _) = send(
        &router,
        post_json("/bookings", Some(&coach), request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, booking) = send(
        &router,
        post_json("/bookings", Some(&student), request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    assert_eq!(booking["status"], "pending");

    // 同じ枠は別の生徒も予約できない
    let (status, body) = send(&router, post_json("/bookings", Some(&other), request)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "slot_already_booked");

    let path = format!(
        "/bookings/{}/confirm",
        booking["booking_id"].as_str().unwrap()
    );
    let (status, body) = send(&router, post_json(&path, Some(&other), json!({}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, confirmed) = send(&router, post_json(&path, Some(&student), json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_eq!(confirmed["status"], "confirmed");
    assert_eq!(confirmed["booking_id"], booking["booking_id"]);

    // 確定した枠は検索に出ない
    let request = Request::get("/slots?sport=tennis")
        .header(header::AUTHORIZATION, format!("Bearer {student}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&router, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"], json!([]));
}

#[tokio::test]
async fn student_cancels_a_confirmed_booking_and_the_slot_is_searchable_again() {
    let router = server::router();
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let (status, slot) = send(
        &router,
        post_json(
            "/slots",
            Some(&coach),
            json!({
                "sport": "tennis",
                "starts_at": "2099-01-01T10:00:00Z",
                "ends_at": "2099-01-01T11:00:00Z",
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{slot}");
    let student = register(&router, "taro@example.com").await;
    let (status, booking) = send(
        &router,
        post_json(
            "/bookings",
            Some(&student),
            json!({ "slot_id": slot["slot_id"], "is_trial": false }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{booking}");
    let booking_id = booking["booking_id"].as_str().unwrap();
    let (status, _) = send(
        &router,
        post_json(
            &format!("/bookings/{booking_id}/confirm"),
            Some(&student),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let cancel = format!("/bookings/{booking_id}/cancel");
    let (status, _) = send(&router, post_json(&cancel, Some(&coach), json!({}))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "コーチはキャンセルできない");
    let (status, cancelled) = send(
        &router,
        post_json(
            &cancel,
            Some(&student),
            json!({ "reason": "都合が悪くなった" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["status"], "cancelled");
    assert!(cancelled["cancelled_at"].is_string());

    // キャンセルで空いた枠は検索に戻る
    let request = Request::get("/slots?sport=tennis")
        .header(header::AUTHORIZATION, format!("Bearer {student}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&router, request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"][0]["slot_id"], slot["slot_id"]);
}

#[tokio::test]
async fn student_lists_only_own_bookings() {
    let router = server::router();
    let coach = MockTokenService
        .issue(TokenClaims {
            account_id: AccountId::from_u128(42),
            role: Role::Coach,
        })
        .into_string();
    let mut slots = Vec::new();
    for hour in ["10", "12"] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                Some(&coach),
                json!({
                    "sport": "tennis",
                    "starts_at": format!("2099-01-01T{hour}:00:00Z"),
                    "ends_at": format!("2099-01-01T{hour}:30:00Z"),
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        slots.push(slot);
    }
    let student = register(&router, "taro@example.com").await;
    let other = register(&router, "hanako@example.com").await;
    let mut booking_ids = Vec::new();
    for (token, slot) in [(&student, &slots[0]), (&other, &slots[1])] {
        let (status, booking) = send(
            &router,
            post_json(
                "/bookings",
                Some(token),
                json!({ "slot_id": slot["slot_id"], "is_trial": true }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        booking_ids.push(booking["booking_id"].clone());
    }

    let get = |token: &str| {
        Request::get("/bookings/me")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    let (status, body) = send(&router, get(&student)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bookings = body["bookings"].as_array().unwrap();
    assert_eq!(bookings.len(), 1, "{body}");
    assert_eq!(bookings[0]["booking"]["booking_id"], booking_ids[0]);
    assert_eq!(bookings[0]["booking"]["status"], "pending");
    assert_eq!(bookings[0]["slot"]["slot_id"], slots[0]["slot_id"]);
    assert_eq!(bookings[0]["slot"]["sport"], "tennis");
    assert_eq!(bookings[0]["slot"]["starts_at"], "2099-01-01T10:00:00Z");

    // コーチは呼べない
    let (status, body) = send(&router, get(&coach)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

#[tokio::test]
async fn coach_lists_only_bookings_of_own_slots_with_student_display_names() {
    let router = server::router();
    let coach_token = |id: u128| {
        MockTokenService
            .issue(TokenClaims {
                account_id: AccountId::from_u128(id),
                role: Role::Coach,
            })
            .into_string()
    };
    let (coach, other_coach) = (coach_token(42), coach_token(43));
    let mut slots = Vec::new();
    for (token, hour) in [(&coach, "12"), (&coach, "10"), (&other_coach, "11")] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                Some(token),
                json!({
                    "sport": "tennis",
                    "starts_at": format!("2099-01-01T{hour}:00:00Z"),
                    "ends_at": format!("2099-01-01T{hour}:30:00Z"),
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        slots.push(slot);
    }
    let student = register(&router, "taro@example.com").await;
    let mut booking_ids = Vec::new();
    for (slot, is_trial) in [(&slots[0], true), (&slots[1], false), (&slots[2], false)] {
        let (status, booking) = send(
            &router,
            post_json(
                "/bookings",
                Some(&student),
                json!({ "slot_id": slot["slot_id"], "is_trial": is_trial }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        booking_ids.push(booking["booking_id"].clone());
    }

    let get = |token: &str| {
        Request::get("/coaches/me/bookings")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    let (status, body) = send(&router, get(&coach)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bookings = body["bookings"].as_array().unwrap();
    let listed: Vec<_> = bookings
        .iter()
        .map(|item| item["booking"]["booking_id"].clone())
        .collect();
    assert_eq!(
        listed,
        [booking_ids[1].clone(), booking_ids[0].clone()],
        "開始時刻の古い順で、別のコーチの枠への予約は出ない: {body}"
    );
    assert_eq!(bookings[0]["slot"]["starts_at"], "2099-01-01T10:00:00Z");
    assert_eq!(bookings[0]["slot"]["status"], "open");
    assert_eq!(bookings[0]["booking"]["status"], "pending");
    assert_eq!(bookings[0]["student_display_name"], "たろう");

    // 生徒は呼べない
    let (status, body) = send(&router, get(&student)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

#[tokio::test]
async fn headquarters_lists_bookings_of_every_coach_by_date_in_japan() {
    let router = server::router();
    let token = |id: u128, role: Role| {
        MockTokenService
            .issue(TokenClaims {
                account_id: AccountId::from_u128(id),
                role,
            })
            .into_string()
    };
    let (coach, other_coach) = (token(42, Role::Coach), token(43, Role::Coach));
    let headquarters = token(1, Role::Headquarters);
    // 2099-01-01T15:00:00Z は日本時間の 1 月 2 日 0 時
    let mut slots = Vec::new();
    for (token, starts_at) in [
        (&coach, "2099-01-01T14:00:00Z"),
        (&other_coach, "2099-01-01T15:00:00Z"),
        (&coach, "2099-01-02T03:00:00Z"),
    ] {
        let (status, slot) = send(
            &router,
            post_json(
                "/slots",
                Some(token),
                json!({
                    "sport": "tennis",
                    "starts_at": starts_at,
                    "ends_at": starts_at.replace(":00:00Z", ":30:00Z"),
                }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{slot}");
        slots.push(slot);
    }
    let student = register(&router, "taro@example.com").await;
    let mut booking_ids = Vec::new();
    for (slot, is_trial) in [(&slots[0], true), (&slots[1], false), (&slots[2], false)] {
        let (status, booking) = send(
            &router,
            post_json(
                "/bookings",
                Some(&student),
                json!({ "slot_id": slot["slot_id"], "is_trial": is_trial }),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{booking}");
        booking_ids.push(booking["booking_id"].clone());
    }

    let get = |path: &str, token: &str| {
        Request::get(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    let (status, body) = send(
        &router,
        get("/bookings?from=2099-01-02&to=2099-01-02", &headquarters),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["from"].clone(), body["to"].clone()),
        (json!("2099-01-02"), json!("2099-01-02"))
    );
    let bookings = body["bookings"].as_array().unwrap();
    let listed: Vec<_> = bookings
        .iter()
        .map(|item| item["booking"]["booking_id"].clone())
        .collect();
    assert_eq!(
        listed,
        [booking_ids[1].clone(), booking_ids[2].clone()],
        "日本時間の 1 月 2 日に開始する予約だけを、開始の古い順に: {body}"
    );
    assert_eq!(bookings[0]["booking"]["status"], "pending");
    assert_eq!(bookings[0]["booking"]["is_trial"], false);
    assert_eq!(bookings[0]["student_display_name"], "たろう");

    // 本部以外は呼べない（生徒の POST /bookings は同じパスのまま使える）
    for token in [&coach, &student] {
        let (status, body) = send(&router, get("/bookings", token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    }
    // 終わりの日付が始まりより前なら 422
    let (status, body) = send(
        &router,
        get("/bookings?from=2099-01-02&to=2099-01-01", &headquarters),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
}
