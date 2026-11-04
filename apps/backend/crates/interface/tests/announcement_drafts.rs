//! `/announcement-drafts` の統合テスト。admin-web の `POST /announcement-drafts`（`createAnnouncementDraft`）・
//! `GET /announcement-drafts`（`listAnnouncementDrafts`）・`PUT` / `DELETE /announcement-drafts/{announcement_draft_id}`
//! （`updateAnnouncementDraft` / `deleteAnnouncementDraft`）・`POST /announcement-drafts/{announcement_draft_id}/send`
//! （`sendAnnouncementDraft`）のレスポンスの形を admin-web の OpenAPI のスキーマで検査し、本部が書きかけの下書きを作り・
//! 直し・消し・一覧で読めること（生徒には届かない）・下書きから送ると `sendAnnouncement` と同じく生徒にお知らせが届き、
//! 下書きが消えて送った記録の一覧に出ること・空の題や本文は送れず 422・対象の生徒がいなければ 404（どちらも下書きは残る）・
//! 長すぎる中身と不正なスポーツは 422・題の一部で一覧を探せること（長すぎる検索語は 422）・最後に直した日付の
//! `from`・`to` で絞れること（日付でない・期間の規則違反は 422）・ない下書きは 404・コーチと生徒の 403・トークンなしの 401 を確かめる。

use std::time::{Duration, SystemTime};

use application::{
    identity::{
        AccessToken, AccountRepository, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier,
        account_id_uuid,
    },
    in_memory::{
        FixedClock, InMemoryAccountRepository, InMemoryAnnouncementDraftRepository,
        InMemoryAnnouncementRepository, InMemoryNotificationRepository,
        InMemoryNotificationSettingsRepository, RecordingPushSender,
    },
    notification::{
        CreateAnnouncementDraft, DeleteAnnouncementDraft, ListAnnouncementDrafts,
        ListAnnouncements, SendAnnouncement, SendAnnouncementDraft, UpdateAnnouncementDraft,
        WithdrawAnnouncement,
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
    notification::Notification,
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

/// テニスの生徒 1・水泳の生徒 2 と、テニスのコーチ 9
const TENNIS_STUDENT: AccountId = AccountId::from_u128(1);
const SWIMMING_STUDENT: AccountId = AccountId::from_u128(2);

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
#[derive(Default)]
struct App {
    drafts: InMemoryAnnouncementDraftRepository,
    announcements: InMemoryAnnouncementRepository,
    notifications: InMemoryNotificationRepository,
    push: RecordingPushSender,
}

impl App {
    /// 時刻 `now` に扱うルーター（下書きと、送った記録の一覧を読む `/announcements`）。
    async fn router(&self, now: SystemTime) -> Router {
        let accounts = InMemoryAccountRepository::default();
        for account in [
            account(TENNIS_STUDENT, Role::Student, &[Sport::Tennis]),
            account(SWIMMING_STUDENT, Role::Student, &[Sport::Swimming]),
            account(AccountId::from_u128(9), Role::Coach, &[Sport::Tennis]),
            account(HEADQUARTERS_ID, Role::Headquarters, &[]),
        ] {
            accounts.insert(account).await.unwrap();
        }
        let send = || {
            SendAnnouncement::new(
                self.announcements.clone(),
                ProfileAnnouncementRecipientLookup::new(accounts.clone()),
                self.notifications.clone(),
                InMemoryNotificationSettingsRepository::default(),
                self.push.clone(),
                FixedClock(now),
            )
        };
        interface::announcement_drafts_router(
            CreateAnnouncementDraft::new(self.drafts.clone(), FixedClock(now)),
            ListAnnouncementDrafts::new(self.drafts.clone()),
            UpdateAnnouncementDraft::new(self.drafts.clone(), FixedClock(now)),
            DeleteAnnouncementDraft::new(self.drafts.clone()),
            SendAnnouncementDraft::new(self.drafts.clone(), send()),
            FakeTokens,
        )
        .merge(interface::announcements_router(
            send(),
            ListAnnouncements::new(self.announcements.clone(), self.notifications.clone()),
            WithdrawAnnouncement::new(
                self.announcements.clone(),
                self.notifications.clone(),
                FixedClock(now),
            ),
            FakeTokens,
        ))
    }
}

const DRAFTS: &str = "/announcement-drafts";
const DRAFT: &str = "/announcement-drafts/{announcement_draft_id}";
const SEND: &str = "/announcement-drafts/{announcement_draft_id}/send";

/// 下書き `id` の `path` の URI。
fn uri(path: &str, id: &str) -> String {
    path.replace("{announcement_draft_id}", id)
}

/// 本部が時刻 `minute` に下書き `body` を作り、作った下書きを返す。
async fn create(app: &App, minute: u64, body: Value) -> Value {
    let (status, created) = call(
        &app.router(at(minute)).await,
        "POST",
        DRAFTS,
        DRAFTS,
        Some(HEADQUARTERS),
        Some(Ok(body)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    created
}

/// 本部が時刻 `minute` に読む下書きの一覧。
async fn list(app: &App, minute: u64) -> Value {
    let (status, body) = call(
        &app.router(at(minute)).await,
        "GET",
        DRAFTS,
        DRAFTS,
        Some(HEADQUARTERS),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
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

#[tokio::test]
async fn headquarters_saves_a_draft_that_reaches_no_student() {
    let app = App::default();
    let created = create(
        &app,
        10,
        json!({ "title": "メンテナンスのお知らせ", "body": "" }),
    )
    .await;
    let id = created["announcement_draft_id"].as_str().unwrap();
    assert_eq!(
        created,
        json!({
            "announcement_draft_id": id,
            "created_by": account_id_uuid::format(HEADQUARTERS_ID),
            "title": "メンテナンスのお知らせ",
            "body": "",
            "sport": null,
            "created_at": "2026-11-01T00:10:00Z",
            "updated_at": "2026-11-01T00:10:00Z"
        }),
        "書きかけの空の本文も残せる。sport を省けば生徒全員"
    );
    assert_eq!(
        list(&app, 20).await,
        json!({ "announcement_drafts": [created] })
    );
    assert!(app.notifications.all().is_empty(), "下書きは生徒に届かない");
    assert!(app.push.sent().is_empty());
    assert!(app.announcements.all().is_empty(), "送った記録にもならない");
}

#[tokio::test]
async fn headquarters_revises_a_draft_and_the_list_is_most_recently_updated_first() {
    let app = App::default();
    let first = create(&app, 10, json!({ "title": "一つ目", "body": "本文" })).await;
    let second = create(
        &app,
        20,
        json!({ "title": "二つ目", "body": "本文", "sport": "tennis" }),
    )
    .await;
    assert_eq!(
        list(&app, 25).await,
        json!({ "announcement_drafts": [second, first] })
    );

    let id = first["announcement_draft_id"].as_str().unwrap();
    let (status, revised) = call(
        &app.router(at(30)).await,
        "PUT",
        &uri(DRAFT, id),
        DRAFT,
        Some("headquarters:6"),
        Some(Ok(
            json!({ "title": "直した題", "body": "直した本文\n2 行目", "sport": "swimming" }),
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{revised}");
    let mut expected = first.clone();
    expected["title"] = json!("直した題");
    expected["body"] = json!("直した本文\n2 行目");
    expected["sport"] = json!("swimming");
    expected["updated_at"] = json!("2026-11-01T00:30:00Z");
    assert_eq!(
        revised, expected,
        "作った本部のアカウントと作った時刻は変えない（ほかの本部のアカウントでも直せる）"
    );
    assert_eq!(
        list(&app, 40).await,
        json!({ "announcement_drafts": [expected, second] }),
        "最後に直した時刻の新しい順"
    );
}

#[tokio::test]
async fn headquarters_deletes_a_draft_and_gets_the_rest() {
    let app = App::default();
    let first = create(&app, 10, json!({ "title": "一つ目", "body": "" })).await;
    let second = create(&app, 20, json!({ "title": "二つ目", "body": "" })).await;
    let id = first["announcement_draft_id"].as_str().unwrap();
    let router = app.router(at(30)).await;
    let (status, body) = call(
        &router,
        "DELETE",
        &uri(DRAFT, id),
        DRAFT,
        Some(HEADQUARTERS),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "announcement_drafts": [second] }));

    let (status, body) = call(
        &router,
        "DELETE",
        &uri(DRAFT, id),
        DRAFT,
        Some(HEADQUARTERS),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "二度目は消すものがない: {body}"
    );
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn sending_a_draft_delivers_it_like_send_announcement_and_removes_the_draft() {
    let app = App::default();
    let draft = create(
        &app,
        10,
        json!({ "title": "水泳の生徒へ", "body": "プールの日程", "sport": "swimming" }),
    )
    .await;
    let id = draft["announcement_draft_id"].as_str().unwrap();
    let router = app.router(at(30)).await;
    let (status, sent) = call(
        &router,
        "POST",
        &uri(SEND, id),
        SEND,
        Some("headquarters:6"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{sent}");
    let mut without_id = sent.clone();
    without_id
        .as_object_mut()
        .unwrap()
        .remove("announcement_id");
    assert_eq!(
        without_id,
        json!({
            "sender_id": account_id_uuid::format(AccountId::from_u128(6)),
            "title": "水泳の生徒へ",
            "body": "プールの日程",
            "sport": "swimming",
            "recipient_count": 1,
            "read_count": 0,
            "sent_at": "2026-11-01T00:30:00Z",
            "withdrawal": null
        }),
        "送ったのは呼んだ本部のアカウント"
    );

    let recipients: Vec<_> = app
        .notifications
        .all()
        .iter()
        .map(Notification::recipient)
        .collect();
    assert_eq!(recipients, [SWIMMING_STUDENT], "対象の生徒にお知らせが届く");
    assert_eq!(app.push.sent().len(), 1);
    assert_eq!(
        list(&app, 40).await,
        json!({ "announcement_drafts": [] }),
        "送った下書きは消える"
    );
    let (_, announcements) = call(
        &router,
        "GET",
        "/announcements",
        "/announcements",
        Some(HEADQUARTERS),
        None,
    )
    .await;
    assert_eq!(
        announcements,
        json!({ "announcements": [sent] }),
        "送った記録の一覧に出る"
    );

    let (status, body) = call(
        &router,
        "POST",
        &uri(SEND, id),
        SEND,
        Some(HEADQUARTERS),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "同じ下書きはもう送れない: {body}"
    );
    assert_eq!(app.announcements.all().len(), 1);
}

#[tokio::test]
async fn a_draft_that_cannot_be_sent_is_kept() {
    let app = App::default();
    for (request, expected_status) in [
        (
            json!({ "title": " ", "body": "本文" }),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            json!({ "title": "題", "body": "" }),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            json!({ "title": "題", "body": "本文", "sport": "golf" }),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let draft = create(&app, 10, request.clone()).await;
        let id = draft["announcement_draft_id"].as_str().unwrap();
        let (status, body) = call(
            &app.router(at(20)).await,
            "POST",
            &uri(SEND, id),
            SEND,
            Some(HEADQUARTERS),
            None,
        )
        .await;
        assert_eq!(status, expected_status, "{request}: {body}");
    }
    assert_eq!(app.drafts.all().len(), 3, "送れなかった下書きは残す");
    assert!(app.announcements.all().is_empty());
    assert!(app.notifications.all().is_empty());
}

#[tokio::test]
async fn an_invalid_draft_is_rejected_and_changes_nothing() {
    let app = App::default();
    let draft = create(&app, 10, json!({ "title": "題", "body": "本文" })).await;
    let id = draft["announcement_draft_id"].as_str().unwrap();
    let router = app.router(at(20)).await;
    for request in [
        Ok(json!({ "title": "あ".repeat(51), "body": "" })),
        Ok(json!({ "title": "", "body": "あ".repeat(2001) })),
        Ok(json!({ "title": "題", "body": "本文", "sport": "cricket" })),
        Ok(json!({ "title": "題" })),
        Ok(json!({ "title": "題", "body": 1 })),
        Err("{"),
    ] {
        for (method, request_uri, path) in [
            ("POST", DRAFTS.to_string(), DRAFTS),
            ("PUT", uri(DRAFT, id), DRAFT),
        ] {
            let (status, body) = call(
                &router,
                method,
                &request_uri,
                path,
                Some(HEADQUARTERS),
                Some(request.clone()),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{method} {request:?}: {body}"
            );
            assert_eq!(body["code"], "validation_failed");
        }
    }
    assert_eq!(
        list(&app, 30).await,
        json!({ "announcement_drafts": [draft] }),
        "何も変えない"
    );

    // 上限ちょうどは通る
    create(
        &app,
        40,
        json!({ "title": "あ".repeat(50), "body": "あ".repeat(2000) }),
    )
    .await;
}

/// 本部が時刻 `minute` に、題の検索語のクエリ `query`（`?q=...`）で読む下書きの一覧の題。
async fn search(app: &App, minute: u64, query: &str) -> (StatusCode, Value) {
    call(
        &app.router(at(minute)).await,
        "GET",
        &format!("{DRAFTS}{query}"),
        DRAFTS,
        Some(HEADQUARTERS),
        None,
    )
    .await
}

fn titles(body: &Value) -> Vec<&str> {
    body["announcement_drafts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|draft| draft["title"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn headquarters_finds_drafts_by_a_part_of_the_title() {
    let app = App::default();
    for (minute, title) in [
        (10, "メンテナンスのお知らせ"),
        (20, "App の更新"),
        (30, ""),
        (40, "夜間メンテナンス 100%"),
    ] {
        create(&app, minute, json!({ "title": title, "body": "本文" })).await;
    }

    let all = vec![
        "夜間メンテナンス 100%",
        "",
        "App の更新",
        "メンテナンスのお知らせ",
    ];
    for (query, expected) in [
        // 題に部分一致するものだけ。並べ方（最後に直した時刻の新しい順）は変わらない
        (
            "?q=%E3%83%A1%E3%83%B3%E3%83%86",
            vec!["夜間メンテナンス 100%", "メンテナンスのお知らせ"],
        ),
        // 前後の空白（全角を含む）を除き、ASCII の英字は大文字・小文字を区別しない
        ("?q=+%E3%80%80APP+", vec!["App の更新"]),
        // % はワイルドカードでなく文字のまま
        ("?q=0%25", vec!["夜間メンテナンス 100%"]),
        // 本文では探さない
        ("?q=%E6%9C%AC%E6%96%87", vec![]),
        // 付けない・空・空白だけなら絞り込まない
        ("", all.clone()),
        ("?q=", all.clone()),
        ("?q=+%E3%80%80+", all.clone()),
        // 前後の空白を除いて 50 文字までは受け付ける
        (&format!("?q=+{}+", "a".repeat(50)), vec![]),
    ] {
        let (status, body) = search(&app, 50, query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(titles(&body), expected, "{query}");
    }

    let (status, body) = search(&app, 50, &format!("?q={}", "a".repeat(51))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_failed");
    assert_eq!(body["message"], "お知らせの題の検索語は 50 文字まで");
}

#[tokio::test]
async fn headquarters_narrows_drafts_by_sport() {
    let app = App::default();
    for (minute, title, sport) in [
        (10, "テニスの大会", json!("tennis")),
        (20, "ゴルフの大会", json!("golf")),
        (30, "テニスの大会と更新", Value::Null),
        (40, "テニスの大会の結果", json!("tennis")),
    ] {
        create(
            &app,
            minute,
            json!({ "title": title, "body": "本文", "sport": sport }),
        )
        .await;
    }

    for (query, expected) in [
        // そのスポーツの生徒に向けた下書きだけ。生徒全員に向けた下書きは入らない。並べ方は変わらない
        ("?sport=tennis", vec!["テニスの大会の結果", "テニスの大会"]),
        ("?sport=golf", vec!["ゴルフの大会"]),
        ("?sport=swimming", vec![]),
        // q と組み合わせられる
        (
            "?sport=tennis&q=%E7%B5%90%E6%9E%9C",
            vec!["テニスの大会の結果"],
        ),
        ("?q=%E6%9B%B4%E6%96%B0&sport=tennis", vec![]),
        ("?sport=golf&q=%E3%83%86%E3%83%8B%E3%82%B9", vec![]),
        // 付けなければ今までどおり
        (
            "",
            vec![
                "テニスの大会の結果",
                "テニスの大会と更新",
                "ゴルフの大会",
                "テニスの大会",
            ],
        ),
    ] {
        let (status, body) = search(&app, 50, query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(titles(&body), expected, "{query}");
    }

    for query in ["?sport=ski", "?sport=", "?sport=Tennis"] {
        let (status, body) = search(&app, 50, query).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
    }
}

#[tokio::test]
async fn headquarters_narrows_drafts_by_the_date_of_the_last_update() {
    let app = App::default();
    // 11/1 の 9 時（日本時間）に 2 つ作り、片方を 11/2 の 0 時に直す。11/3 の 0 時にもう 1 つ作る
    create(&app, 10, json!({ "title": "古い下書き", "body": "本文" })).await;
    let revised = create(&app, 20, json!({ "title": "直す下書き", "body": "本文" })).await;
    let (status, body) = call(
        &app.router(at(900)).await,
        "PUT",
        &uri(DRAFT, revised["announcement_draft_id"].as_str().unwrap()),
        DRAFT,
        Some(HEADQUARTERS),
        Some(Ok(json!({ "title": "直した下書き", "body": "本文" }))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    create(
        &app,
        2340,
        json!({ "title": "新しい下書き", "body": "本文" }),
    )
    .await;

    for (query, expected) in [
        // 最後に直した日付（日本時間）で絞る。直した下書きは作った日付では入らない
        ("?from=2026-11-01&to=2026-11-01", vec!["古い下書き"]),
        (
            "?from=2026-11-02&to=2026-11-03",
            vec!["新しい下書き", "直した下書き"],
        ),
        // 片方だけなら、もう片方から 7 日
        ("?from=2026-11-02", vec!["新しい下書き", "直した下書き"]),
        ("?to=2026-11-02", vec!["直した下書き", "古い下書き"]),
        ("?from=2026-11-04", vec![]),
        // q と sport と組み合わせられる
        (
            "?from=2026-11-01&to=2026-11-03&q=%E7%9B%B4%E3%81%97%E3%81%9F",
            vec!["直した下書き"],
        ),
        ("?from=2026-11-01&to=2026-11-03&sport=tennis", vec![]),
        // 付けなければ今までどおり
        ("", vec!["新しい下書き", "直した下書き", "古い下書き"]),
        // 31 日まではよい
        (
            "?from=2026-11-01&to=2026-12-01",
            vec!["新しい下書き", "直した下書き", "古い下書き"],
        ),
    ] {
        let (status, body) = search(&app, 3000, query).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(titles(&body), expected, "{query}");
    }

    for (query, message) in [
        (
            "?from=2026-11-03&to=2026-11-02",
            "to は from と同じ日か、それより後の日付にする",
        ),
        (
            "?from=2026-11-01&to=2026-12-02",
            "from から to までは 31 日まで",
        ),
        ("?from=2026/11/01", "from は YYYY-MM-DD の日付でない"),
        ("?to=", "to は YYYY-MM-DD の日付でない"),
    ] {
        let (status, body) = search(&app, 3000, query).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}: {body}");
        assert_eq!(body["code"], "validation_failed", "{query}");
        assert_eq!(body["message"], message, "{query}");
    }
}

#[tokio::test]
async fn a_missing_draft_is_not_found() {
    let app = App::default();
    let router = app.router(at(10)).await;
    for id in ["00000000-0000-4000-8000-000000000099", "not-a-uuid"] {
        for (method, path, body) in [
            ("PUT", DRAFT, Some(Ok(json!({ "title": "題", "body": "" })))),
            ("DELETE", DRAFT, None),
            ("POST", SEND, None),
        ] {
            let (status, response) = call(
                &router,
                method,
                &uri(path, id),
                path,
                Some(HEADQUARTERS),
                body,
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{method} {id}: {response}");
            assert_eq!(response["code"], "not_found");
        }
    }
    assert!(app.drafts.all().is_empty());
}

#[tokio::test]
async fn only_headquarters_can_call_and_a_token_is_required() {
    let app = App::default();
    let draft = create(&app, 10, json!({ "title": "題", "body": "本文" })).await;
    let id = draft["announcement_draft_id"].as_str().unwrap();
    let router = app.router(at(20)).await;
    let content = || Some(Ok(json!({ "title": "題", "body": "本文" })));
    let operations = [
        ("POST", DRAFTS.to_string(), DRAFTS, content()),
        ("GET", DRAFTS.to_string(), DRAFTS, None),
        ("PUT", uri(DRAFT, id), DRAFT, content()),
        ("DELETE", uri(DRAFT, id), DRAFT, None),
        ("POST", uri(SEND, id), SEND, None),
    ];
    for (method, request_uri, path, body) in &operations {
        for token in ["coach:9", "student:1"] {
            let (status, response) = call(
                &router,
                method,
                request_uri,
                path,
                Some(token),
                body.clone(),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{token} {method} {path}: {response}"
            );
            assert_eq!(response["code"], "forbidden");
        }
        let (status, response) = call(&router, method, request_uri, path, None, body.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{method} {path}: {response}"
        );
        assert_eq!(response["code"], "unauthorized");
    }
    assert_eq!(app.drafts.all().len(), 1, "断ったときは何も変えない");
    assert!(app.announcements.all().is_empty());
}
