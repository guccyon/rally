//! `/announcements` の統合テスト。admin-web の `POST /announcements`（`sendAnnouncement`）と `GET /announcements`
//! （`listAnnouncements`）と `POST /announcements/{announcement_id}/withdraw`（`withdrawAnnouncement`）のレスポンスの形を
//! admin-web の OpenAPI のスキーマで検査し、本部が生徒全員か、スポーツを絞った生徒にお知らせを送れること（生徒ごとに
//! お知らせを作り、受け取り設定に従ってプッシュ通知を送る）・送った記録を新しい順に読めること
//! （1 件ごとに受け取った生徒のうち既読にした数。取り下げたものは取り下げた時刻まで）・送った日付（日本時間）の
//! `from`・`to` で絞れること（日付でない・期間の規則違反は 422）・題の `q` で絞れること（期間と組み合わせられる。
//! 長すぎる検索語は 422）・対象の生徒がいなければ 404・
//! 題と本文とスポーツの規則違反は 422・送ったお知らせを取り下げられること（一覧で取り下げ済みと分かる。二度目は 409、
//! ないものは 404）・コーチと生徒の 403・トークンなしの 401 を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryAnnouncementRepository,
        InMemoryNotificationRepository, InMemoryNotificationSettingsRepository,
        RecordingPushSender,
    },
    notification::{
        ListAnnouncements, NotificationRepository, SendAnnouncement, WithdrawAnnouncement,
    },
    profile::ProfileAnnouncementRecipientLookup,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use domain::{
    catalog::Sport,
    identity::{Account, AccountId, Email, Role},
    notification::{Notification, NotificationKind, NotificationSettings},
    profile::DisplayName,
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

const HEADQUARTERS: &str = "headquarters:5";
const HEADQUARTERS_ID: AccountId = AccountId::from_u128(5);

/// テニスの生徒 1・水泳の生徒 2・テニスの生徒 3（本部からのお知らせのプッシュ通知をオフ）と、テニスのコーチ 9
const TENNIS_STUDENT: AccountId = AccountId::from_u128(1);
const SWIMMING_STUDENT: AccountId = AccountId::from_u128(2);
const QUIET_STUDENT: AccountId = AccountId::from_u128(3);

/// 2026-11-01T00:00:00Z から `minute` 分後
fn at(minute: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200 + minute * 60)
}

fn account(id: AccountId, role: Role, sports: &[Sport]) -> Account {
    Account::new(
        id,
        Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
        DisplayName::new("テスト").unwrap(),
        role,
        sports.iter().copied(),
        at(0),
    )
    .unwrap()
}

/// 組み立てたルーターと、テストが中身を読む保存先・送り先。
struct App {
    announcements: InMemoryAnnouncementRepository,
    notifications: InMemoryNotificationRepository,
    push: RecordingPushSender,
}

impl App {
    async fn new() -> Self {
        Self {
            announcements: InMemoryAnnouncementRepository::default(),
            notifications: InMemoryNotificationRepository::default(),
            push: RecordingPushSender::default(),
        }
    }

    /// 時刻 `now` に送るルーター。
    async fn router(&self, now: SystemTime) -> Router {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(TENNIS_STUDENT, Role::Student, &[Sport::Tennis]),
            account(SWIMMING_STUDENT, Role::Student, &[Sport::Swimming]),
            account(QUIET_STUDENT, Role::Student, &[Sport::Tennis]),
            account(AccountId::from_u128(9), Role::Coach, &[Sport::Tennis]),
            account(HEADQUARTERS_ID, Role::Headquarters, &[]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let mut quiet = NotificationSettings::new(QUIET_STUDENT);
        quiet.set(NotificationKind::Announcement, false);
        interface::announcements_router(
            SendAnnouncement::new(
                self.announcements.clone(),
                ProfileAnnouncementRecipientLookup::new(accounts),
                self.notifications.clone(),
                InMemoryNotificationSettingsRepository::with_settings([quiet]),
                self.push.clone(),
                FixedClock(now),
            ),
            ListAnnouncements::new(self.announcements.clone(), self.notifications.clone()),
            WithdrawAnnouncement::new(
                self.announcements.clone(),
                self.notifications.clone(),
                FixedClock(now),
            ),
            FakeTokens,
        )
    }
}

/// `token` で呼び、レスポンスを admin-web の契約で検査する。`body` が `None` なら本文なし、`Some(Err)` なら JSON でない本文。
async fn send(
    router: &Router,
    method: &str,
    token: Option<&str>,
    body: Option<Result<Value, &str>>,
) -> (StatusCode, Value) {
    call(
        router,
        method,
        "/announcements",
        "/announcements",
        token,
        body,
    )
    .await
}

/// `token` で `announcement_id` のお知らせを取り下げ、レスポンスを admin-web の契約で検査する。
async fn withdraw(
    router: &Router,
    token: Option<&str>,
    announcement_id: &str,
) -> (StatusCode, Value) {
    call(
        router,
        "POST",
        &format!("/announcements/{announcement_id}/withdraw"),
        "/announcements/{announcement_id}/withdraw",
        token,
        None,
    )
    .await
}

/// `token` で `uri` を呼び、レスポンスを admin-web の契約の `path` で検査する。
async fn call(
    router: &Router,
    method: &str,
    uri: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Result<Value, &str>>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(body) => {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(match body {
                Ok(json) => json.to_string(),
                Err(text) => text.to_string(),
            })
        }
        None => Body::empty(),
    };
    let response = router
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap();
    support::Contract::load("admin-web").assert_response(
        &method.to_lowercase(),
        path,
        status,
        &body,
    );
    (status, body)
}

fn maintenance() -> Value {
    json!({ "title": "メンテナンスのお知らせ", "body": "11/3 の 2:00〜4:00 は使えません。\nご不便をおかけします。" })
}

/// 送ったお知らせの中身（ID を除く）。
fn sent(body: &Value) -> Value {
    let mut body = body.clone();
    body.as_object_mut().unwrap().remove("announcement_id");
    body
}

#[tokio::test]
async fn headquarters_sends_to_every_student_and_respects_the_settings() {
    let app = App::new().await;
    let router = app.router(at(10)).await;
    let (status, body) = send(&router, "POST", Some(HEADQUARTERS), Some(Ok(maintenance()))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        sent(&body),
        json!({
            "sender_id": account_id_uuid::format(HEADQUARTERS_ID),
            "title": "メンテナンスのお知らせ",
            "body": "11/3 の 2:00〜4:00 は使えません。\nご不便をおかけします。",
            "sport": null,
            "recipient_count": 3,
            "read_count": 0,
            "sent_at": "2026-11-01T00:10:00Z",
            "withdrawal": null
        }),
        "コーチ・本部は数えない"
    );

    let announcement = app.announcements.all().remove(0);
    let recipients: Vec<_> = app
        .notifications
        .all()
        .iter()
        .map(Notification::recipient)
        .collect();
    let mut sorted = recipients.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        [TENNIS_STUDENT, SWIMMING_STUDENT, QUIET_STUDENT],
        "生徒全員にお知らせを作る（受け取り設定でオフの生徒にも）"
    );
    assert!(
        app.notifications
            .all()
            .iter()
            .all(|n| n.push() == announcement.push_to(n.recipient()) && n.created_at() == at(10))
    );
    let mut pushed: Vec<_> = app.push.sent().iter().map(|p| p.recipient()).collect();
    pushed.sort();
    assert_eq!(
        pushed,
        [TENNIS_STUDENT, SWIMMING_STUDENT],
        "プッシュ通知はオフの生徒には送らない"
    );
}

#[tokio::test]
async fn a_sport_narrows_the_students() {
    let app = App::new().await;
    let router = app.router(at(10)).await;
    let mut request = maintenance();
    request["sport"] = json!("swimming");
    let (status, body) = send(&router, "POST", Some(HEADQUARTERS), Some(Ok(request))).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["sport"], "swimming");
    assert_eq!(body["recipient_count"], 1);
    let recipients: Vec<_> = app
        .notifications
        .all()
        .iter()
        .map(Notification::recipient)
        .collect();
    assert_eq!(recipients, [SWIMMING_STUDENT]);
}

#[tokio::test]
async fn headquarters_lists_what_was_sent_newest_first() {
    let app = App::new().await;
    let (status, body) = send(&app.router(at(0)).await, "GET", Some(HEADQUARTERS), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "announcements": [] }));

    let (_, first) = send(
        &app.router(at(10)).await,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(maintenance())),
    )
    .await;
    let (_, second) = send(
        &app.router(at(20)).await,
        "POST",
        Some("headquarters:6"),
        Some(Ok(
            json!({ "title": "キャンペーン", "body": "テニスの生徒へ", "sport": "tennis" }),
        )),
    )
    .await;

    let (status, body) = send(&app.router(at(30)).await, "GET", Some(HEADQUARTERS), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "announcements": [second, first] }),
        "新しい順。ほかの本部のアカウントが送った分も出す"
    );
    assert_eq!(app.announcements.all().len(), 2, "照会では何も変えない");
}

/// `token` で `GET /announcements?{query}` を呼び、レスポンスを admin-web の契約で検査する。
async fn list(router: &Router, query: &str) -> (StatusCode, Value) {
    call(
        router,
        "GET",
        &format!("/announcements?{query}"),
        "/announcements",
        Some(HEADQUARTERS),
        None,
    )
    .await
}

#[tokio::test]
async fn the_sent_date_in_japan_narrows_the_list() {
    let app = App::new().await;
    // 2026-11-01T00:10:00Z は日本時間の 11/1、2026-11-01T15:00:00Z は日本時間の 11/2 の始まり
    let (_, nov1) = send(
        &app.router(at(10)).await,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(maintenance())),
    )
    .await;
    let (_, nov2) = send(
        &app.router(at(15 * 60)).await,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(maintenance())),
    )
    .await;
    let router = app.router(at(16 * 60)).await;

    for (query, expected) in [
        ("", json!([nov2, nov1])),
        ("from=2026-11-01&to=2026-11-01", json!([nov1])),
        ("from=2026-11-01&to=2026-11-02", json!([nov2, nov1])),
        ("from=2026-11-02", json!([nov2])),
        ("to=2026-11-01", json!([nov1])),
        ("from=2026-11-03", json!([])),
    ] {
        let (status, body) = list(&router, query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(body, json!({ "announcements": expected }), "{query}");
    }
}

#[tokio::test]
async fn headquarters_finds_sent_announcements_by_a_part_of_the_title() {
    let app = App::new().await;
    // 日本時間の 11/1 に 2 件、11/2 に 1 件
    let mut sent = Vec::new();
    for (minute, title) in [
        (10, "メンテナンスのお知らせ"),
        (20, "App の更新"),
        (15 * 60, "夜間メンテナンス 100%"),
    ] {
        let (status, body) = send(
            &app.router(at(minute)).await,
            "POST",
            Some(HEADQUARTERS),
            Some(Ok(json!({ "title": title, "body": "本文" }))),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        sent.push(body);
    }
    let [maintenance, update, night] = <[Value; 3]>::try_from(sent).unwrap();
    let router = app.router(at(16 * 60)).await;

    let all = json!([night, update, maintenance]);
    for (query, expected) in [
        // 題に部分一致するものだけ。並べ方（新しい順）は変わらない
        ("q=%E3%83%A1%E3%83%B3%E3%83%86", json!([night, maintenance])),
        // 前後の空白（全角を含む）を除き、ASCII の英字は大文字・小文字を区別しない
        ("q=+%E3%80%80APP+", json!([update])),
        // % はワイルドカードでなく文字のまま
        ("q=0%25", json!([night])),
        // 本文では探さない
        ("q=%E6%9C%AC%E6%96%87", json!([])),
        // 期間と組み合わせられる（日本時間の 11/1 に送ったもののうち）
        (
            "from=2026-11-01&to=2026-11-01&q=%E3%83%A1%E3%83%B3%E3%83%86",
            json!([maintenance]),
        ),
        ("to=2026-11-02&q=app", json!([update])),
        // 付けない・空・空白だけなら絞り込まない
        ("", all.clone()),
        ("q=", all.clone()),
        ("q=+%E3%80%80+", all.clone()),
        // 前後の空白を除いて 50 文字までは受け付ける
        (&format!("q=+{}+", "a".repeat(50)), json!([])),
    ] {
        let (status, body) = list(&router, query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(body, json!({ "announcements": expected }), "{query}");
    }

    let (status, body) = list(&router, &format!("q={}", "a".repeat(51))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
    assert_eq!(body["message"], "お知らせの題の検索語は 50 文字まで");
}

#[tokio::test]
async fn an_invalid_period_is_rejected() {
    let app = App::new().await;
    let router = app.router(at(0)).await;
    for (query, message) in [
        (
            "from=2026-11-03&to=2026-11-02",
            "to は from と同じ日か、それより後の日付にする",
        ),
        (
            "from=2026-11-01&to=2026-12-02",
            "from から to までは 31 日まで",
        ),
        ("from=2026/11/01", "from は YYYY-MM-DD の日付でない"),
        ("to=", "to は YYYY-MM-DD の日付でない"),
    ] {
        let (status, body) = list(&router, query).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
        assert_eq!(body["message"], message, "{query}");
    }
    let (status, _) = list(&router, "from=2026-11-01&to=2026-12-01").await;
    assert_eq!(status, StatusCode::OK, "31 日まではよい");
}

#[tokio::test]
async fn the_list_counts_the_students_who_read_each_announcement() {
    let app = App::new().await;
    let (_, everyone) = send(
        &app.router(at(10)).await,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(maintenance())),
    )
    .await;
    let (_, swimming) = send(
        &app.router(at(20)).await,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(
            json!({ "title": "キャンペーン", "body": "水泳の生徒へ", "sport": "swimming" }),
        )),
    )
    .await;
    // 生徒 1 は全員へのお知らせだけを、生徒 2 は両方を読む（生徒 3 は読まない）
    app.notifications
        .mark_all_read(TENNIS_STUDENT, at(30))
        .await
        .unwrap();
    app.notifications
        .mark_all_read(SWIMMING_STUDENT, at(30))
        .await
        .unwrap();

    let (status, body) = send(&app.router(at(40)).await, "GET", Some(HEADQUARTERS), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let counts: Vec<_> = body["announcements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["announcement_id"].clone(),
                a["recipient_count"].clone(),
                a["read_count"].clone(),
            )
        })
        .collect();
    assert_eq!(
        counts,
        [
            (swimming["announcement_id"].clone(), json!(1), json!(1)),
            (everyone["announcement_id"].clone(), json!(3), json!(2)),
        ],
        "受け取った生徒のうち既読にした数"
    );

    // 取り下げても一覧に残り、既読にした数は取り下げた時刻までのもの
    let id = everyone["announcement_id"].as_str().unwrap();
    let (status, withdrawn) = withdraw(&app.router(at(50)).await, Some(HEADQUARTERS), id).await;
    assert_eq!(status, StatusCode::OK, "{withdrawn}");
    assert_eq!(withdrawn["read_count"], 2, "取り下げの応答にも載る");
    app.notifications
        .mark_all_read(QUIET_STUDENT, at(60))
        .await
        .unwrap();
    let (_, listed) = send(&app.router(at(70)).await, "GET", Some(HEADQUARTERS), None).await;
    assert_eq!(listed["announcements"][1]["announcement_id"], id);
    assert_eq!(
        listed["announcements"][1]["read_count"], 2,
        "取り下げたあとに「すべて既読にする」を押した生徒は数えない"
    );
    assert_eq!(
        app.notifications
            .all()
            .iter()
            .filter(|n| n.recipient() == QUIET_STUDENT)
            .map(|n| n.is_read())
            .collect::<Vec<_>>(),
        [true],
        "既読そのものは変えない（D141）"
    );
}

#[tokio::test]
async fn no_students_of_the_sport_is_not_found_and_records_nothing() {
    let app = App::new().await;
    let router = app.router(at(10)).await;
    let mut request = maintenance();
    request["sport"] = json!("golf");
    let (status, body) = send(&router, "POST", Some(HEADQUARTERS), Some(Ok(request))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], "not_found");
    assert!(app.announcements.all().is_empty());
    assert!(app.notifications.all().is_empty());
    assert!(app.push.sent().is_empty());
}

#[tokio::test]
async fn an_invalid_request_is_rejected_and_records_nothing() {
    let app = App::new().await;
    let router = app.router(at(10)).await;
    for request in [
        Ok(json!({ "title": " ", "body": "本文" })),
        Ok(json!({ "title": "あ".repeat(51), "body": "本文" })),
        Ok(json!({ "title": "題", "body": "\n" })),
        Ok(json!({ "title": "題", "body": "あ".repeat(2001) })),
        Ok(json!({ "title": "題", "body": "本文", "sport": "cricket" })),
        Ok(json!({ "title": "題" })),
        Ok(json!({ "title": "題", "body": 1 })),
        Err("{"),
    ] {
        let (status, body) = send(&router, "POST", Some(HEADQUARTERS), Some(request.clone())).await;
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{request:?}: {body}"
        );
        assert_eq!(body["code"], "validation_failed");
    }
    assert!(app.announcements.all().is_empty());
    assert!(app.notifications.all().is_empty());

    // 上限ちょうどは通る
    let (status, body) = send(
        &router,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(
            json!({ "title": "あ".repeat(50), "body": "あ".repeat(2000) }),
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn headquarters_withdraws_an_announcement_and_the_list_shows_it() {
    let app = App::new().await;
    let (_, first) = send(
        &app.router(at(10)).await,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(maintenance())),
    )
    .await;
    let (_, second) = send(
        &app.router(at(20)).await,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(json!({ "title": "キャンペーン", "body": "本文" }))),
    )
    .await;
    let id = first["announcement_id"].as_str().unwrap();

    let (status, body) = withdraw(&app.router(at(30)).await, Some("headquarters:6"), id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mut expected = first.clone();
    expected["withdrawal"] = json!({
        "withdrawn_by": account_id_uuid::format(AccountId::from_u128(6)),
        "withdrawn_at": "2026-11-01T00:30:00Z"
    });
    assert_eq!(
        body, expected,
        "送った中身は変えず、取り下げた本部のアカウント（送った本部でなくてよい）と時刻を載せる"
    );

    let (_, listed) = send(&app.router(at(40)).await, "GET", Some(HEADQUARTERS), None).await;
    assert_eq!(
        listed,
        json!({ "announcements": [second, expected] }),
        "一覧には取り下げたものも残り、取り下げ済みと分かる"
    );
    assert_eq!(
        app.notifications.all().len(),
        6,
        "生徒ごとのお知らせは消さない（生徒の一覧で外す）"
    );
    assert_eq!(
        app.push.sent().len(),
        4,
        "取り下げてもプッシュ通知は送らない"
    );
}

#[tokio::test]
async fn withdrawing_twice_is_a_conflict_and_keeps_the_first_withdrawal() {
    let app = App::new().await;
    let (_, sent) = send(
        &app.router(at(10)).await,
        "POST",
        Some(HEADQUARTERS),
        Some(Ok(maintenance())),
    )
    .await;
    let id = sent["announcement_id"].as_str().unwrap();
    let (_, first) = withdraw(&app.router(at(20)).await, Some(HEADQUARTERS), id).await;

    let (status, body) = withdraw(&app.router(at(30)).await, Some("headquarters:6"), id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "announcement_already_withdrawn");
    let (_, listed) = send(&app.router(at(40)).await, "GET", Some(HEADQUARTERS), None).await;
    assert_eq!(listed["announcements"][0], first, "最初の取り下げのまま");
}

#[tokio::test]
async fn a_missing_announcement_is_not_found() {
    let app = App::new().await;
    let router = app.router(at(10)).await;
    send(&router, "POST", Some(HEADQUARTERS), Some(Ok(maintenance()))).await;
    for id in ["00000000-0000-4000-8000-000000000099", "not-a-uuid"] {
        let (status, body) = withdraw(&router, Some(HEADQUARTERS), id).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}: {body}");
        assert_eq!(body["code"], "not_found");
    }
    assert!(!app.announcements.all()[0].is_withdrawn(), "何も変えない");
}

#[tokio::test]
async fn only_headquarters_can_withdraw() {
    let app = App::new().await;
    let router = app.router(at(10)).await;
    let (_, sent) = send(&router, "POST", Some(HEADQUARTERS), Some(Ok(maintenance()))).await;
    let id = sent["announcement_id"].as_str().unwrap();
    for token in ["coach:9", "student:1"] {
        let (status, body) = withdraw(&router, Some(token), id).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{token}: {body}");
        assert_eq!(body["code"], "forbidden");
    }
    let (status, body) = withdraw(&router, None, id).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "unauthorized");
    assert!(
        !app.announcements.all()[0].is_withdrawn(),
        "断ったときは何も変えない"
    );
}

#[tokio::test]
async fn only_headquarters_can_call_and_a_token_is_required() {
    let app = App::new().await;
    let router = app.router(at(10)).await;
    for token in ["coach:9", "student:1"] {
        for (method, body) in [("POST", Some(Ok(maintenance()))), ("GET", None)] {
            let (status, response) = send(&router, method, Some(token), body).await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{token} {method}: {response}"
            );
            assert_eq!(response["code"], "forbidden");
        }
    }
    for (method, body) in [("POST", Some(Ok(maintenance()))), ("GET", None)] {
        let (status, response) = send(&router, method, None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method}: {response}");
        assert_eq!(response["code"], "unauthorized");
    }
    assert!(
        app.announcements.all().is_empty(),
        "断ったときは何も残さない"
    );
    assert!(app.notifications.all().is_empty());
}
