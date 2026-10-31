# apps/backend

Rally のバックエンド（Rust / axum / PostgreSQL）の Cargo ワークスペース。層の規約と依存方向は [ADR 0003](../../docs/adr/0003-backend-layers.md)。

## 構成

| パス | 層 | 依存してよいクレート |
|---|---|---|
| `crates/domain/` | ドメイン。純粋（I/O なし）。`identity`（アカウント・役割と不変条件）・`profile`（表示名 `DisplayName`・コーチの自己紹介文 `CoachBio`）・`scheduling`（スロットと、重なりを判定する集約 `CoachCalendar`・予約 `Booking`・キャンセル規定 `CancellationPolicy`・スポーツごとの予約枠の需給 `SlotSupply`・コーチの受付 `CoachAcceptance`）・`entitlement`（契約種別 `ContractType`・受講権 `Entitlement`・通話の暦月の枠 `LiveLessonQuota`・暦月 `YearMonth`・月額契約 `Subscription`・契約プラン `SubscriptionPlan`）・`live_lesson`（通話セッション `LiveLessonSession`・接続品質の生の指標 `ConnectionQuality`）・`async_lesson`（動画 `Video`・添削 `VideoReview`・添削コメント `ReviewComment`・動画の形式 `VideoContentType`・AI の一次フィードバック `AiFeedback`）・`lesson`（live-lesson と async-lesson が共有するレッスンの完了記録 `LessonCompletion`・`LessonCompletionId`・`LessonType`・`LessonSource`・完了したレッスンのメニューの写し `LessonCompletionMenu`・コーチの完了したレッスンの種類ごとの数 `CoachLessonCount`。コンテキストではない）・`notification`（プッシュ通知 `PushNotification`・通知の種類 `NotificationKind`・通知の受け取り設定 `NotificationSettings`・お知らせ `Notification`・`NotificationId`・お知らせの不変条件の違反 `NotificationError`）・`catalog`（スポーツ `Sport`・コーチのレッスンメニュー（glossary の lesson）の集約 `LessonMenu`・`LessonMenuId`・中身 `LessonMenuContent`・名前 `LessonMenuName`・説明 `LessonMenuDescription`・形式 `LessonFormat`（同期は 1 回の時間 `LessonDuration` を持つ））・`matching`（生徒のお気に入りのコーチ `FavoriteCoach`・お気に入りの不変条件の違反 `MatchingError`） | なし |
| `crates/application/` | ユースケースとポート（`identity::RegisterAccount`・`GetAccount`・`ListStudents`（本部が見る生徒の一覧）・`StudentContractLookup`（実装は `entitlement::EntitlementStudentContractLookup`）・`StudentLastBookingLookup`（実装は `scheduling::SchedulingStudentLastBookingLookup`）・`AccountRepository`・`TokenIssuer`・`TokenVerifier`・`profile::ListCoaches`・`GetCoachOverview`（本部が見るコーチ 1 人の様子。受付の状態と、`GetCoachReviewSummary`・`GetMyLessonStats`・`ListCoachLessonMenus` の結果を組み合わせる）・`UpdateCoachProfile`・`ProfileStudentDisplayNameLookup`（scheduling・async-lesson に生徒の表示名を渡す）・`scheduling::PublishSlot`・`WithdrawSlot`（コーチの枠の取り下げ）・`SuspendCoach`・`ResumeCoach`（本部によるコーチの受付の停止・再開）・`CoachAcceptanceRepository`・`SchedulingCoachAcceptanceLookup`（matching の `SuspendedCoachLookup` と profile の `CoachAcceptanceLookup` の実装）・`SearchSlots`・`SlotRepository`・`CoachIntroductionLookup`（スロットの検索に添えるコーチの紹介。実装は `profile::ProfileCoachIntroductionLookup`）・`LessonMenuLookup`（枠に結びつけるレッスンメニューの確認と、枠の一覧に添えるメニューと、予約に写すメニュー。実装は `catalog::CatalogLessonMenuLookup`）・`CreateBooking`・`ConfirmBooking`・`CancelBooking`・`GetSlotSupply`・`ListMyBookings`（自分の予約の一覧）・`BookingRepository`・`QuotaGate`・`entitlement::GetEntitlement`・`StartSubscription`・`ConsumeLiveLessonQuota`・`ReleaseLiveLessonQuota`・`LiveLessonQuotaSubscriber`・`EntitlementQuotaGate`・`EntitlementRepository`・`live_lesson::StartLiveLessonSession`・`EndLiveLessonSession`・`LiveLessonSessionRepository`・`VideoCallProvider`・`BookingLookup`（実装は `scheduling::SchedulingBookingLookup`）・`async_lesson::SubmitVideo`・`GetVideoReview`・`ReturnVideoReview`・`VideoRepository`・`VideoStorage`・`AiFeedbackGenerator`（AI の一次フィードバック。材料は `AiFeedbackRequest`）・`VideoLessonMenuLookup`（送付で選んだ非同期のレッスンメニューの確認と、動画に写すメニュー。実装は `catalog::CatalogLessonMenuLookup`）・`ReviewGate`（実装は `entitlement::EntitlementReviewGate`）・`lesson::RecordLessonCompletion`・`LessonCompletionRepository`・`GetMyLessonStats`（コーチの今月と先月のレッスンの実績）・`MonthlyReviewSummaryLookup`（実績に添える暦月ごとの評価の件数と平均。実装は `review::ReviewMonthlyReviewSummaryLookup`）・`ListMyLessonCompletions`（生徒が受けたレッスンの一覧。コーチの表示名 `LessonCompletionCoachDisplayNameLookup` は `profile::ProfileCoachDisplayNameLookup`、評価済みの印 `LessonCompletionReviewLookup` は `review::ReviewLessonCompletionReviewLookup` が実装）・`notification::PushNotificationSubscriber`・`PushSender`・`GetNotificationSettings`・`UpdateNotificationSettings`・`NotificationSettingsRepository`・`NotificationRepository`・`ListMyNotifications`・`MarkMyNotificationsRead`・`catalog::CreateLessonMenu`・`UpdateLessonMenu`・`ListMyLessonMenus`・`ListCoachLessonMenus`（公開中だけ）・`ListLessonMenus`（本部が見るすべてのコーチの横断の一覧）・`LessonMenuCoachDisplayNameLookup`（横断の一覧に添えるコーチの表示名。実装は `profile::ProfileCoachDisplayNameLookup`）・`LessonMenuRepository`・`AnalyticsSink`・`DomainEventPublisher`・`DomainEventSubscriber`・`Clock`）。ポートのインメモリ実装（`in_memory`）。ID と UUID の変換（`identity::account_id_uuid`・`scheduling::slot_id_uuid`・`scheduling::booking_id_uuid`・`entitlement::entitlement_id_uuid`・`entitlement::subscription_id_uuid`・`live_lesson::session_id_uuid`・`async_lesson::video_id_uuid`・`async_lesson::video_review_id_uuid`・`lesson::lesson_completion_id_uuid`・`catalog::lesson_menu_id_uuid`・`notification::notification_id_uuid`）・時刻から暦月と暦月の始まりの時刻（`entitlement::calendar_month`。日本時間）・障害の型（`Unavailable`） | domain |
| `crates/infra/` | Postgres（`migrations/`・`PgSlotRepository`・`PgCoachAcceptanceRepository`・起動時に `MIGRATOR` を当てる `run_migrations`（DB に当たっていて知らない版は飛ばす））・モックの外部連携（`MockTokenService`: アクセストークン、`LoggingDomainEventPublisher`: 境界間イベントを標準出力に書く、`InMemoryDomainEventBus`: 境界間イベントを同じプロセスの購読側に渡す、`MockVideoCallProvider`: 通話基盤（Twilio Video 相当）。終了時に接続品質の指標を乱数で生成する、`MockAltVideoCallProvider`: 別の通話基盤を想定したモック。生の値の単位が違い、アダプタの中でポートの単位に換算する、`MockVideoStorage`: 動画の保存先。固定の形のアップロード URL を返す、`MockAiFeedbackGenerator`: AI の一次フィードバック。スポーツごとの決まった文を返し、外に出ない、`MockPushSender`: プッシュ通知の送り先。送った通知を記録して標準出力に書く、`ObjectStoreAnalyticsSink`: 行動トラッキングの配信先。S3 API のオブジェクトストアに JSON Lines で書く）・DB とトークンと境界間イベント・行動イベントのペイロードの文字列の対応表（`identity_values`・`scheduling_values`・`catalog_values`・`domain_event_payload`・`analytics_event_payload`） | domain・application |
| `crates/interface/` | axum の HTTP（`GET /healthz`、`GET /version`（`version::router`）、`POST /accounts`、`GET /slots`、`POST /slots`、`POST /bookings`、`GET /bookings/me`、`GET /coaches/me/bookings`、`GET /coaches/me/videos`、`POST /bookings/{booking_id}/confirm`、`POST /bookings/{booking_id}/cancel`、`GET /slot-supply`、`GET /coaches`、`GET /me`、`PUT /me/profile`、`POST /subscriptions`、`GET /entitlements/me`、`GET /notification-settings/me`、`PUT /notification-settings/me`、`POST /live-lesson-sessions`、`POST /live-lesson-sessions/{session_id}/end`、`POST /videos`、`GET /videos/{video_id}/review`、`POST /videos/{video_id}/review`、`POST /lessons/completions`。catalog は別のルーター `catalog_router` で `GET`・`POST /coaches/me/lesson-menus`、`PUT /coaches/me/lesson-menus/{lesson_menu_id}`、`GET /coaches/{coach_id}/lesson-menus`、`GET /lesson-menus`（本部）、生徒が送った動画の一覧は別のルーター `my_videos_router` で `GET /videos/me`、生徒が受けたレッスンの一覧は別のルーター `my_lesson_completions_router` で `GET /lessons/completions/me`、生徒が見るコーチの紹介は別のルーター `coach_profile_router` で `GET /coaches/{coach_id}`、生徒の自分のプロフィールは別のルーター `my_profile_router` で `GET`・`PATCH /me/profile`、生徒とコーチのお知らせは別のルーター `notifications_router` で `GET /notifications/me`・`POST /notifications/me/read`、本部からのお知らせは別のルーター `announcements_router` で `POST`・`GET /announcements`、コーチの枠の取り下げは別のルーター `slot_withdrawal_router` で `POST /coaches/me/slots/{slot_id}/withdraw`、コーチの今月と先月のレッスンの実績は別のルーター `lesson_stats_router` で `GET /coaches/me/lesson-stats`、本部によるコーチの受付の停止・再開は別のルーター `coach_acceptance_router` で `POST /coaches/{coach_id}/suspend`・`POST /coaches/{coach_id}/resume`、本部が見るコーチ 1 人の様子は別のルーター `coach_overview_router` で `GET /coaches/{coach_id}/overview`、生徒のお気に入りのコーチは別のルーター `favorite_coaches_router` で `GET /favorite-coaches`・`PUT`・`DELETE /favorite-coaches/{coach_id}`）・認証（`AuthenticatedAccount`・`require_role`）・契約の文字列の対応表と API の文言 | domain・application（infra は不可） |
| `crates/server/` | 組み立て用（composition root）。interface に infra の実装を渡し、HTTP サーバーを起動する。行動イベントの配信先（`AnalyticsDestination`）と通話基盤（`VideoCallBackend`）を環境変数で選ぶ。保存先をすべて Postgres にした組み立ては `router_with_postgres`（一周シナリオの E2E が使う）と、通話基盤も選ぶ `router_with_postgres_and_video_call`（`DATABASE_URL` があるときの `main` が使う） | domain・application・infra・interface |
| `crates/architecture/` | 依存方向の検査（テストだけ） | なし |

ツールチェーンは `rust-toolchain.toml`（stable、rustfmt・clippy 付き）。

## データベース

ローカルの Postgres は `compose.yaml`（Docker Compose）で起動する。スキーマは `crates/infra/migrations/` の sqlx のマイグレーションで、変更は expand → 移行 → contract の 3 段に分ける（[ADR 0004](../../docs/adr/0004-schema-expand-contract.md)）。

```sh
docker compose up -d --wait                       # 127.0.0.1:5432 に Postgres 16、127.0.0.1:9090 にオブジェクトストア（下の「行動イベントの配信先」）
export DATABASE_URL=postgres://rally@127.0.0.1:5432/rally
cargo install sqlx-cli --no-default-features --features postgres,rustls   # 初回だけ
cargo sqlx migrate run --source crates/infra/migrations
cargo sqlx migrate info --source crates/infra/migrations
```

マイグレーションのファイル名は `<14 桁の版>_<段>_<説明>.sql`（段は `expand` / `migrate` / `contract`）。規約は `crates/infra/tests/migration_conventions.rs` が `cargo test` で検査する。

スキーマの検証（`crates/infra/tests/schema.rs`）は Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` を設定して次で走らせる（テストごとに新しい DB を作ってマイグレーションを当てる）。

```sh
cargo test -p infra -- --include-ignored
```

## 起動

```sh
cargo run -p server               # 127.0.0.1:8080 で待ち受ける
BIND_ADDR=0.0.0.0:3000 cargo run -p server
curl -i http://127.0.0.1:8080/healthz   # 200 ok
curl -i -X POST http://127.0.0.1:8080/accounts -H 'content-type: application/json' \
  -d '{"email":"taro@example.com","display_name":"たろう","sport":"tennis"}'   # 201
DATABASE_URL=postgres://rally@127.0.0.1:5432/rally APP_VERSION=0.1.0 cargo run -p server   # 保存先を Postgres にする
curl -i http://127.0.0.1:8080/version   # 200 {"app_version":"0.1.0"}
```

ポートは組み立て用のクレート `server` で組み立てる（ADR 0003 の決定 5）。

| 環境変数 | 意味 |
|---|---|
| `DATABASE_URL` | あれば保存先をすべて Postgres にし（`server::router_with_postgres_and_video_call`）、待ち受ける前に `infra::run_migrations` で `infra::MIGRATOR` を当てる。DB に当たっていて自分の知らないマイグレーション（新しい版が当てたもの）は飛ばして起動する（前の版のイメージに戻す Rollback のため。[ADR 0004](../../docs/adr/0004-schema-expand-contract.md) の 1）。DB につなげない・マイグレーションを当てられない（当て済みのものが書き換えられていたなど）ときは、原因を標準エラー出力に書いて 0 以外で終わる。なければ保存先は `application::in_memory` のインメモリ実装で、登録したアカウントや公開した枠はプロセスを止めると消える |
| `APP_VERSION` | `GET /version`（認証なし）が `{"app_version": "..."}` で返す版。ないか空なら `"unknown"`。出荷した版を確かめる運用の口で、`/healthz` と同じく生徒・本部の API の契約（`specs/contracts`）には載せない |

起動の試験（バイナリを起動して HTTP で確かめる）は `crates/server/tests/startup.rs`。Postgres を使うものは `DATABASE_URL` を設定して `cargo test -p server --test startup -- --include-ignored` で走らせる。
行動イベントの配信先は環境変数 `ANALYTICS_SINK` で選ぶ（既定はインメモリ。下の「行動イベントの配信先」）。
通話基盤は環境変数 `VIDEO_CALL_PROVIDER` で選ぶ（既定は `twilio`。下の「通話基盤の切り替え」）。

## identity の不変条件

`domain::identity`（表示名・自己紹介文は `domain::profile`）がモデルの生成時に検証し、破れば `DomainError` を返す。

- 役割（生徒・コーチ・本部）とスポーツの組み合わせ: 生徒はちょうど 1 つ、コーチは 1 つ以上（提供スポーツの集合）、本部はなし
- スポーツは提供スポーツの 6 種類（Blueprint の tennis / golf / baseball / soccer / basketball と、変更シナリオ cs-001 で足した swimming）のどれか。student-app の `POST /accounts` はさらにゴルフを 422 で断る（契約の `Sport` がゴルフを含まない）。水泳は DB の sport の CHECK 制約の付け直し（`20261028000003_expand_swimming.sql`）で足し、月額（ゴルフ以外）の規則（`ContractType::covers`）は変えずに月額に入る。水泳の一周（コーチ・枠・月額の受講権・予約）は `crates/server/tests/swimming.rs`（Postgres。`--include-ignored`）
- コーチの自己紹介文（profile の `CoachBio`）は空でもよく 400 文字まで（契約の `maxLength`。前後の空白も削らずに持つ）。登録したばかりのコーチは空
- 表示名（profile の `DisplayName`）は空（空白だけ）でなく 50 文字まで（契約の `maxLength`）。メールアドレスは `@` の前後が空でない最小の形で、一意性は大文字小文字を区別しない

domain は列挙型と値（`AccountId` は `u128`）だけを持ち、外部の文字列表現を知らない。対応表と文言は境界に置く。

| 何の文字列か | 置き場所 |
|---|---|
| 契約（OpenAPI）の `Sport` の値 | `interface` の `contract.rs` |
| API の `message`（`DomainError` の変種・役割の呼び名から組み立てる） | `interface` の `message.rs` |
| messaging の失敗（`domain::messaging::MessagingError`）の API の `message`・HTTP の状態・`code` | `interface` の `messaging_error.rs`（`From<MessagingError> for ApiError`。ハンドラーは変種を個別に拾わない） |
| review の失敗（`domain::review::ReviewError`）の API の `message`・HTTP の状態・`code` | `interface` の `review_error.rs`（`From<ReviewError> for ApiError`。ハンドラーは変種を個別に拾わない） |
| entitlement の失敗（`domain::entitlement::EntitlementError` と、受講権がない 404・すでにある 409）の API の `message`・HTTP の状態・`code` | `interface` の `entitlement_error.rs`（`From<EntitlementError> for ApiError` と `entitlement_not_found`・`already_subscribed`・`live_lesson_quota_exceeded`。予約の確定の 409 `live_lesson_quota_exceeded` の応答もここ。ハンドラーは変種を個別に拾わない） |
| scheduling の失敗（`domain::scheduling::SchedulingError` と、予約の作成・確定・キャンセルと枠の公開のユースケースの失敗）の API の `message`・HTTP の状態・`code` | `interface` の `scheduling_error.rs`（`From<SchedulingError>`・`From<CreateBookingError>`・`From<ConfirmBookingError>`・`From<CancelBookingError>`・`From<PublishSlotError>`・`From<ListBookingsError>` for `ApiError`。同じ変種でも操作で応答が違うのでユースケースの失敗ごとに写し、どれも変種を漏れなく並べる。ハンドラーは変種を個別に拾わない） |
| DB の `identity.accounts.role` / `sport` の値・モックのトークンの役割 | `infra::identity_values`（CHECK との一致は `crates/infra/tests/schema.rs`） |
| アカウント ID の UUID のテキスト形式（契約の `format: uuid`・トークン） | `application::identity::account_id_uuid`（`uuid` クレートに任せる。interface と infra の両方から使うので application に置く） |

保存先の障害は `application::Unavailable`（原因を `source` に持つ）で返す。interface は 500 `internal` に固定の文言だけを返し、原因はクライアントに出さない（標準エラー出力に書く。統合テストは `crates/interface/tests/internal_error.rs`）。

`POST /accounts` の統合テスト（`crates/interface/tests/accounts.rs`）は、レスポンスを `specs/contracts/api/student-app/openapi.yaml` のスキーマで検査する。生徒の登録で行動イベント `account.registered` が 1 回発火することもここで確かめる。トークンは `TokenIssuer` が発行する（このテストではテスト用の偽物）。

## profile: プロフィールの置き場所

- プロフィール（profile。docs/adr/0009）は domain の `profile`（`DisplayName`・`CoachBio`）と application の `profile`（`ListCoaches`・`GetCoachProfile`・`GetCoachBio`・`UpdateCoachProfile`・`ProfileStudentDisplayNameLookup`・`ProfileCoachIntroductionLookup` と、ポート `CoachProfileRepository`）に置く。identity に残るのは登録・本人のアカウントの照会・認証で、`Account` は表示名を profile の型として持つ。`Coach` は提供スポーツだけを持ち、自己紹介文は持たない（D130）
- **profile のデータは当面 identity の表にある**（`identity.accounts.display_name`・`identity.coaches.bio`）。自己紹介文は profile のポート `CoachProfileRepository` で読み書きし、表示名・提供スポーツは identity のポート `AccountRepository` から読む（実装はどちらも `PgAccountRepository`。インメモリは `InMemoryAccountRepository`）。表を分けるのは別の段

## profile: 本部向けのコーチの一覧

- `GET /coaches`（admin-web の `listCoaches`）: 本部が呼べる（コーチは 403）。同じパスを生徒が呼ぶと student-app の `listCoachesForStudent`（下の matching）の形で返す（ルーターは `interface::coaches_router`。`require_role(&[Role::Headquarters, Role::Student])` のうえで、ハンドラーがトークンの役割で形を選ぶ）。コーチの役割のアカウントの `coach_id` と `display_name` を返す。絞り込み・ページングはせず、行動イベントは発火しない
- 並べ方の正本は契約（表示名の Unicode のコードポイントの順、同じ表示名は `coach_id` の順）。保存先のポート `AccountRepository::list_coaches` が並べて返す。Postgres は `ORDER BY display_name COLLATE "C", account_id`（DB の照合順序に依らない。UTF-8 のバイト順はコードポイントの順と同じ）、インメモリは `(表示名の文字列, AccountId)` の順で、両方が同じ並びになる。読んだ行は `Account`（提供スポーツを含む）に戻す
- テスト: `application::profile::list_coaches`（コーチだけ・並べ方）、`crates/interface/tests/coaches.rs`（契約の形・並べ方・403 / 401）、`crates/infra/tests/pg_account_repository.rs`（Postgres。照合順序に依らない並びとスポーツの復元。`--include-ignored`）

## identity: 本部が見る生徒の一覧（D118）

- `GET /students`（admin-web の `listStudents`）: 本部だけが呼べる（`interface::students_router`。`require_role(&[Role::Headquarters])`。コーチ・生徒は 403）。生徒の役割のアカウントごとに `student_id`・表示名・スポーツ・受講権の契約種別 `contract_type` と契約プラン `plan`（受講権がなければどちらも null。ワンショットは `plan` が null）・最後に予約を作った時刻 `last_booked_at`（予約の状態を問わない。なければ null）・登録した時刻 `registered_at` を返す。メールアドレスは返さない。絞り込み・ページング・件数の上限はなく、行動イベントは発火しない（読み取りだけ）
- 並べ方の正本は契約（登録の新しい順、同じ時刻は `student_id` の順）。保存先のポート `AccountRepository::list_students` が並べて返す（Postgres は `ORDER BY created_at DESC, account_id`）
- ユースケースは `application::identity::ListStudents`。他のコンテキストの表は直接読まず、ポートで生徒の ID をまとめて問い合わせる: 受講権の契約種別と契約プラン `StudentContractLookup`（実装は `entitlement::EntitlementStudentContractLookup`。`EntitlementRepository::find_by_student` を生徒ごとに読む）、最後に予約した時刻 `StudentLastBookingLookup`（実装は `scheduling::SchedulingStudentLastBookingLookup`。`BookingRepository::last_booked_at_of_students`。Postgres は `scheduling.bookings` の `max(created_at)` を生徒ごとに 1 回の問い合わせで）。DB の変更はない
- テスト: `application::identity::list_students`（生徒だけ・並べ方・まとめて 1 回の問い合わせ・生徒がいなければ問い合わせない・障害）と各ポートの実装の単体テスト、`crates/interface/tests/students.rs`（契約の形・本部 200・空・コーチ / 生徒 403・401・500 に理由を出さない）、`crates/infra/tests/pg_account_repository.rs`・`pg_booking_repository.rs`（Postgres の並び・最後に予約した時刻。`--include-ignored`）、`crates/server/tests/students.rs`（Postgres の組み立てで登録 → 月額契約 → 予約が一覧に出る。`--include-ignored`）。パスパラメータもクエリも受けないので 404・422 はない

## review: 本部が見る低い評価の一覧（D122）

- `GET /lesson-reviews/low`（admin-web の `listLowRatedLessonReviews`）: 本部だけが呼べる（`interface::low_rated_reviews_router`。`require_role(&[Role::Headquarters])`。コーチ・生徒は 403）。直近 30 日（照会した時刻の 30 × 24 時間前（含む）から照会した時刻（含む）まで）に送られた★2 以下の評価を、コーチを問わず送った時刻の新しい順（同じ時刻は `review_id` の順）に返す。各評価に `review_id`・`lesson_id`・コーチの ID と表示名・生徒の ID と表示名（引けなければ表示名の項目を載せない）・点・一言・送った時刻・レッスンメニューの写し（なければ null）を載せ、一覧にしきい値 `max_rating` と期間 `period_days` を添える。ページング・件数の上限はなく、行動イベントは発火しない（読み取りだけ）
- しきい値（2）と期間（30 日）の正本は契約で、実装は domain の `review::LowRatedReviewCriteria` の 1 か所（選び方と並べ方も `LowRatedReviewCriteria::select`）。interface はレスポンスの `max_rating` / `period_days` にその定数をそのまま載せる
- ユースケースは `application::review::ListLowRatedReviews`。保存先のポート `LessonReviewRepository::find_submitted_since`（期間の始まり以降の評価を点で絞らずに読む。Postgres は `submitted_at >= $1`）で読み、domain で選ぶ。メニューは評価の一覧と同じ `ReviewedLessonMenuLookup`（実装は `lesson::LessonCompletionMenuLookup`）、表示名はポート `review::ReviewedCoachDisplayNameLookup` / `review::ReviewerDisplayNameLookup`（実装は `profile::ProfileCoachDisplayNameLookup` / `profile::ProfileStudentDisplayNameLookup`）で、載せる評価のコーチ・生徒の ID をまとめて問い合わせる。DB の変更はない
- テスト: `domain::review::low_rated_review_criteria`（★2 と★3 の境目・30 日の両端・並べ方）、`application::review::list_low_rated_reviews`（コーチを問わない・メニューと表示名・低い評価がなければ表示名を問い合わせない）、`crates/interface/tests/low_rated_reviews.rs`（契約の形・本部 200・空・コーチ / 生徒 403・401・500 に理由を出さない）、`crates/infra/tests/pg_lesson_review_repository.rs`（Postgres の `find_submitted_since` の境目と一覧。`--include-ignored`）、`crates/server/tests/low_rated_reviews.rs`（Postgres の組み立てで一覧と 403。`--include-ignored`）。パスパラメータもクエリも受けないので 404・422 はない

## review: 評価へのコーチの返信（D131）

- `PUT /lesson-reviews/{review_id}/reply`（admin-web の `replyToLessonReview`）: コーチだけが呼べる（`interface::review_replies_router`。`require_role(&[Role::Coach])`。本部・生徒は 403）。返信できるのは評価されたコーチだけで、他のコーチは 403（評価は一覧で誰でも読めるので、あることは隠さない）。評価 1 件に返信は 1 つで、2 回目は文と時刻を上書きする。返信の削除はない。ない評価・UUID でない `review_id` は 404、空・200 文字を超える返信は 422。レスポンスは返信（`comment`・`replied_at`）。行動イベントは発火しない
- 返信できるかと上書きは domain の `review::LessonReview::write_reply`、文の規則（空でない・200 文字まで）は `review::LessonReviewReplyComment` の 1 か所。ユースケースは `application::review::ReplyToLessonReview`（保存先のポート `LessonReviewRepository::find_by_id` で読み、`save_reply` で返信だけを書く）
- 返信は評価の一覧（admin-web・student-app の `listCoachReviews`、admin-web の `listLowRatedLessonReviews`）の評価ごとの `reply` に載る（なければ null）
- DB は `review.lesson_reviews` に `reply_comment` / `replied_at` を足す expand（`20261101000002_expand_lesson_review_reply.sql`。ADR 0004 の 3）
- テスト: `domain::review::lesson_review`・`lesson_review_reply_comment`（評価されたコーチだけ・上書き・文の境目）、`application::review::reply_to_lesson_review`（保存・上書き・403 にあたる拒否・ない評価）、`crates/interface/tests/review_replies.rs`（契約の形・200・上書き・他のコーチ / 本部 / 生徒 403・401・404・422・500 に理由を出さない）、`crates/interface/tests/reviews.rs`・`low_rated_reviews.rs`（一覧の `reply`）、`crates/infra/tests/pg_lesson_review_repository.rs`・`schema.rs`（Postgres の保存と上書き・制約。`--include-ignored`）、`crates/server/tests/coach_reviews.rs`（Postgres の組み立てで返信して生徒の一覧に載る。`--include-ignored`）

## profile: コーチの自己紹介文（D18）

- `PUT /me/profile`（admin-web の `updateMyCoachProfile`）: コーチだけが呼べる（`require_role(&[Role::Coach])`。本部・生徒は 403）。トークンのコーチの自己紹介文を `bio` で丸ごと置き換え、`GET /me` と同じ形の `Account` を返す。`bio` がない・文字列でない・400 文字を超えると 422（何も変えない）。トークンのコーチのアカウントがなければ `GET /me` と同じく 401。行動イベントは発火しない
- `GET /me`（`getMyAccount`）はコーチのときだけ `bio` を返す（書いていなければ空文字列）。本部・生徒の応答には載らない
- 自己紹介文は `profile::CoachBio` で、ユースケースは `application::profile::UpdateCoachProfile`（書き換え）と `GetCoachBio`（`GET /me` に添える）。保存先は profile のポート `CoachProfileRepository`（`find_coach_bio`・`update_coach_bio`。コーチのアカウントがなければ読み書きとも `None`）。`/me` のハンドラーは identity の `GetAccount` で読んだアカウントに自己紹介文を添えて応答を組み立てる。Postgres は `identity.coaches.bio`（マイグレーション `20261028000002_expand_coach_bio.sql`。既定は空、`coaches_bio_length` で 400 文字まで）
- テスト: `crates/domain/src/profile/coach_bio.rs`（空・上限・超過）、`application::profile::update_coach_profile`（置き換え・空に戻す・超過で変えない・コーチ以外）・`get_coach_bio`（書いた文・空・コーチ以外）、`crates/infra/tests/pg_account_repository.rs`（Postgres の往復・コーチ以外は読み書きとも `None`）・`schema.rs`（既定の空・`coaches_bio_length`）、`crates/interface/tests/me.rs`（契約の形・コーチ / 本部 / 生徒・422・401）、`crates/server/tests/coach_profile.rs`（Postgres の組み立てで保存される。`--include-ignored`）

## matching: 生徒がスポーツでコーチを探す（D96）

- `GET /coaches`（student-app の `listCoachesForStudent`）: 生徒が呼ぶ（本部は admin-web の `listCoaches` の形、コーチは 403）。`sport`（省いたら生徒自身のスポーツ）を提供するコーチを、`coach_id`・表示名・スポーツ・評価の件数と平均・そのスポーツの公開中のレッスンメニューの数で返す。ページングはせず、行動イベントは発火しない（読み取りだけ）
- 並べ方の正本は契約（評価の平均の高い順、評価のないコーチは最後。同じなら表示名の Unicode のコードポイントの順、同じ表示名は `coach_id` の順）。並べるのはユースケース `application::matching::ListCoachesForStudent`
- ゴルフは golf-app の範囲: `sport=golf` は 422（student-app の `Sport` にない）。生徒自身のスポーツがゴルフ（student-app からは登録できない）なら空の配列。`sport` を省いて生徒のアカウントがなければ 401
- 各項目に、生徒のお気に入り（下の D126）に入っているか `is_favorite` を添える（お気に入りの保存先 `FavoriteCoachRepository::find_by_student` を 1 回読む）
- matching の保存先はお気に入りだけで、ほかは他のコンテキストの持ち物をポートで読む: 生徒のスポーツ `StudentSportLookup`（実装は `profile::ProfileStudentSportLookup`）、提供スポーツと表示名 `CoachCandidateLookup`（`profile::ProfileCoachCandidateLookup`）、評価の件数と平均 `CoachReviewSummaryLookup`（`review::ReviewCoachReviewSummaryLookup`。数え方は domain の `CoachReviewSummary::tally`）、公開中のメニューの数 `PublishedLessonMenuCountLookup`（`catalog::CatalogPublishedLessonMenuCountLookup`）。DB の変更はない
- テスト: `application::matching::list_coaches_for_student`（絞り込み・並べ方・生徒がいない）と各ポートの実装の単体テスト、`crates/interface/tests/coaches.rs`（両方の契約の形・生徒 200・`sport` の絞り込み・422・コーチ 403・401）

## matching: 生徒のお気に入りのコーチ（D126）

- `PUT /favorite-coaches/{coach_id}`（student-app の `addFavoriteCoach`）・`DELETE /favorite-coaches/{coach_id}`（`removeFavoriteCoach`）・`GET /favorite-coaches`（`listFavoriteCoaches`）: 生徒だけが呼べる（`interface::favorite_coaches_router`。`require_role(&[Role::Student])`。コーチ・本部は 403）。入れる・外すは、変えたあとの一覧を返す。一覧の各項目は `coach_id`・表示名・提供スポーツ（ゴルフは載せない）・評価の件数と平均・入れた時刻 `favorited_at` で、入れた時刻の新しい順（同じ時刻は `coach_id` の順）。行動イベントは発火しない
- 同じコーチを何度入れても 1 件で、`favorited_at` は最初のまま（Postgres は `matching.favorite_coaches` の主キー `(student_id, coach_id)` に `ON CONFLICT DO NOTHING`）。外すのは何度でもでき、入っていないコーチを外しても 200
- 入れる: コーチでない ID・ない ID・ゴルフだけのコーチ（`getCoachProfile` が 404 のコーチ）・UUID でない ID は 404、トークンの生徒のアカウントがなければ 401。生徒がコーチを入れるかは domain の `matching::FavoriteCoach::new`（`MatchingError::NotAStudent` / `NotACoach`）、ゴルフを除くのはユースケース `application::matching::AddFavoriteCoach` に境界が渡す student-app のスポーツ（`AddFavoriteCoachCommand::sports`）
- お気に入りは matching の持ち物（Postgres の `matching` スキーマ。マイグレーション `20261101000001_expand_favorite_coaches.sql`。生徒は役割つきの外部キー、相手は `identity.coaches`）。入れる・一覧のコーチの表示名と提供スポーツは catalog の `CreateLessonMenu` と同じく `AccountRepository::find` で読み、評価の件数と平均は `CoachReviewSummaryLookup`
- テスト: `crates/domain/src/matching/favorite_coach.rs`（生徒だけ・相手はコーチだけ）、`application::matching::add_favorite_coach`（1 件で最初の時刻・ない / ゴルフだけのコーチ・生徒でない・コーチでない）・`remove_favorite_coach`・`list_favorite_coaches`（並べ方・表示名・評価・ないコーチは載せない）、`crates/interface/tests/favorite_coaches.rs`（契約の形・200・404・401・403）、`crates/infra/tests/pg_favorite_coach_repository.rs`・`schema.rs`（Postgres の往復・組で 1 件・役割の外部キー。`--include-ignored`）、`crates/server/tests/favorite_coaches.rs`（Postgres の組み立てで `is_favorite` まで通す。`--include-ignored`）

## profile: 生徒が見るコーチの紹介（D98）

- `GET /coaches/{coach_id}`（student-app の `getCoachProfile`）: 生徒だけが呼べる（`require_role(&[Role::Student])`。コーチ・本部は 403）。コーチの `coach_id`・`display_name`・`bio`（書いていなければ null）・`sports`（提供スポーツを `Sport` の順で。ゴルフは載せない）・`is_favorite`（トークンの生徒のお気に入りか。profile のポート `CoachFavoriteLookup` を matching の `MatchingCoachFavoriteLookup` が実装する）を返す。行動イベントは発火しない
- コーチでない ID・ない ID・UUID でない ID・ゴルフだけのコーチは 404。ゴルフを除くのは境界（interface）で、ユースケース `application::profile::GetCoachProfile` は `AccountRepository::find` で読んだコーチの紹介（`CoachProfile`。ゴルフも含む）をそのまま返す。ルーターは `catalog_router` などと同じく別の `coach_profile_router`
- テスト: `application::profile::get_coach_profile`（コーチの紹介・空の自己紹介文・コーチ以外は `None`）、`crates/interface/tests/coach_profiles.rs`（契約の形・null の `bio`・ゴルフを除く・404・403 / 401）、`crates/server/tests/coach_profile.rs`（Postgres の組み立てで書いた自己紹介文が出る。`--include-ignored`）

## profile: 生徒の自分の表示名（D100）

- `GET /me/profile`（student-app の `getMyStudentProfile`）・`PATCH /me/profile`（`updateMyStudentProfile`）: 生徒だけが呼べる（`require_role(&[Role::Student])`。コーチ・本部は 403）。応答は登録（`POST /accounts`）の `student` と同じ `Student`（`student_id`・`display_name`・`sport`・`created_at`。形は `accounts::student_body` の 1 か所）。`PATCH` は `display_name` だけを置き換える。表示名の規則は登録と同じ domain の `profile::DisplayName`（空・空白だけ・50 文字超は 422。何も変えない）。`display_name` がない・文字列でないときも 422。トークンの生徒のアカウントがなければ 401。行動イベントは発火しない
- 取得のユースケースは admin-web の `GET /me` と同じ `application::identity::GetAccount`、変更は `application::profile::UpdateStudentProfile`（admin-web の `UpdateCoachProfile` の生徒の分）。保存先はポート `AccountRepository::update_student_display_name`（生徒のアカウントがなければ何も変えずに `None`）。Postgres は既存の `identity.accounts.display_name` を書き換えるだけで、DB の変更はない
- 同じパスの `PUT /me/profile`（コーチ）は `router_with`、生徒の `GET`・`PATCH` は別のルーター `interface::my_profile_router` にし、`server` が `merge` する（axum は同じパスのメソッドを 1 つにまとめる）
- テスト: `application::profile::update_student_profile`（置き換え・上限ちょうど・空 / 空白だけ / 超過で変えない・生徒以外）、`crates/interface/tests/my_profile.rs`（契約の形・取得・変更・422・403・401）、`crates/infra/tests/pg_account_repository.rs`（Postgres の往復・生徒以外は変えない）、`crates/server/tests/student_profile.rs`（Postgres の組み立てで保存される・コーチの `PUT /me/profile` と並んで動く。`--include-ignored`）

## 認証と認可（モック）

- アクセストークンはアカウント ID と役割を載せた不透明な文字列で、`Authorization: Bearer <access_token>` で送る（契約の `bearerAuth`）。発行・検証は application のポート `identity::TokenIssuer` / `TokenVerifier`、実装は infra の `MockTokenService` だけ
- `MockTokenService` は**署名なし**の固定形式 `mock.<役割>.<アカウント ID>` で、誰でも任意の役割のトークンを作れる。Gen1 のローカル起動とテスト用で、本番では署名付きの実装に差し替える。パスワード・外部 IdP・失効は扱わない
- `POST /accounts` と `GET /healthz` 以外のエンドポイントは認証必須にする。ハンドラーの引数に `interface::AuthenticatedAccount` を取れば、トークンがない・無効なら 401 `unauthorized`（`WWW-Authenticate: Bearer`）。役割で絞るルートは `route_layer(interface::require_role(&[Role::Coach]))` を重ね、役割が合わなければ 403 `forbidden`（admin-web の本部とコーチの同居。ADR 0002 Q3）
- 401 / 403 / 200 の統合テストは `crates/server/tests/auth.rs`（infra のモックで発行したトークンと、`POST /accounts` が返したトークンを使う）

## scheduling: スロットの公開と検索

- `POST /slots`（admin-web の `publishSlot`。`specs/contracts/api/admin-web/openapi.yaml`）: コーチだけが呼べる（`require_role(&[Role::Coach])`。生徒・本部は 403）。枠のコーチはトークンのアカウント。公開すると行動イベント `slot.published` を 1 回発火する
- `POST /coaches/me/slots/{slot_id}/withdraw`（admin-web の `withdrawMySlot`）: コーチだけが呼べる。取り下げの規則は domain の `Slot::withdraw` の 1 か所（自分の・公開中の・有効な予約がない・まだ始まっていない枠だけを `closed` にする）で、ユースケース `WithdrawSlot` は判定しない。他のコーチの枠（`SlotOfAnotherCoach`）とない枠は 404、予約が入っている（`SlotHasActiveBookings`）は 409 `slot_has_active_booking`、始まった（`SlotAlreadyStarted`）は 409 `slot_already_started`、閉じた枠は 409 `slot_already_closed`。取り下げると行動イベント `slot.withdrawn` を 1 回発火する。保存先の `SlotRepository::update` は、Postgres ではスロットの行を `FOR UPDATE` でロックしてから席を数える（予約の保存先と同じ `lock_slot`）ので、並行な予約と取り下げの両方が通ることはない
- `GET /slots`（student-app の `searchSlots`）: 認証した利用者なら誰でも呼べる。公開中（`open`）の枠をスポーツ（必須。ゴルフは 422）・コーチ・開始時刻の期間（`starts_after` 以上 `starts_before` 未満。`starts_after` の既定は現在時刻）で絞り、（開始時刻, スロット ID）の昇順で返す。`limit` は 1〜100（既定 20）、`next_cursor` は次のページの位置（クライアントには不透明。形は `interface` の `contract.rs`）
- 不変条件は `domain::scheduling` が持ち、破れば `domain::scheduling::SchedulingError` を返す（枠・定員・席・カレンダー・予約・枠へのメニューの結びつけ。集約・値オブジェクトと、ユースケースの `PublishSlotError::Invalid`・`WithdrawSlotError::Invalid`・`CreateBookingError::Invalid`・`ConfirmBookingError::Invalid`・`CancelBookingError::Invalid`、保存先のクロージャもこれ）: 開始 < 終了（期間は終了を含まない）、Gen1 の定員は 1、同じコーチの時間を占める枠（`open` / `booked`）は重ならない（集約 `CoachCalendar`。`closed` は時間を占めない）。重なりは 409 `slot_overlaps`
- 重なりの判定は domain だけに置き、DB の排他制約は置かない。Postgres の実装（`infra::PgSlotRepository`）はコーチごとの advisory lock で並行な公開を直列にする（ADR 0004 の 4）
- `server` はスロットの保存先も当面インメモリ（`InMemorySlotRepository`）。`scheduling.slots.coach_id` が `identity.coaches` を指すので、アカウントの保存先を Postgres にするときに一緒に差し替える
- 統合テスト: `crates/interface/tests/slots.rs`（契約の形・403・絞り込み・ページング）、`crates/server/tests/slots.rs`（モックのトークンで通す）、`crates/infra/tests/pg_slot_repository.rs`（Postgres。重なりの拒否と並行な公開・検索・取り下げと並行な予約。`--include-ignored`）、`crates/interface/tests/slot_withdrawals.rs`（取り下げの 200・404・409・403）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `Sport`（admin-web は 6 種類、student-app はゴルフを除く 5 種類）・`SlotStatus`・`date-time`・`next_cursor` | `interface` の `contract.rs` |
| DB の `scheduling.slots.status` の値 | `infra::scheduling_values`（CHECK との一致は `crates/infra/tests/schema.rs`） |
| スロット ID の UUID のテキスト形式 | `application::scheduling::slot_id_uuid` |

## scheduling: 予約の作成と確定（体験予約を含む）

- `POST /bookings`（student-app の `createBooking`）: 生徒だけが呼べる（`require_role(&[Role::Student])`。コーチ・本部は 403）。予約する生徒はトークンのアカウント。作成直後は `pending` で、スロットはまだ `open` のまま（受講権も消費しない）。作成すると行動イベント `booking.created` を 1 回発火する
- `POST /bookings/{booking_id}/confirm`（`confirmBooking`）: 生徒だけが呼べ、確定できるのは自分の予約だけ（別の生徒の予約・UUID でない ID は 404）。月額の生徒でその暦月のビデオ通話の枠が残っていなければ 409 `live_lesson_quota_exceeded`（下の entitlement の節）。確定するとスロットが `booked` になり（検索に出なくなる）、境界間イベント `BookingConfirmed` を発行し、行動イベント `booking.confirmed` を 1 回発火する。`pending` でなければ 409 `invalid_booking_state`
- 不変条件は `domain::scheduling::Booking` が持つ: 公開中（`open`）のスロットにしか予約できない（予約済みは 409 `slot_already_booked`、閉じた枠はない枠と同じ 404）、1 スロットに有効な（キャンセルされていない）予約は 1 件（二重予約は 409 `slot_already_booked`）、体験予約はその生徒の初回だけ（キャンセルされていない予約が 1 件でもあれば 409 `trial_already_used`）
- ポート `BookingRepository` は予約とスロットを一緒に読み書きし、判定は domain に任せて読み取りから保存までを直列にする。`server` は当面インメモリ（`InMemoryBookingRepository`。スロットの保存先と中身を共有）。Postgres の実装はアカウント・スロットの保存先を Postgres にするときに足す（DB は `bookings_one_active_per_slot` でも二重予約を防ぐ）
- 境界間イベントはポート `DomainEventPublisher`（発行の失敗でユースケースを失敗させない）。Gen1 はメッセージ基盤を選定していないので、`server` は infra の `InMemoryDomainEventBus`（`LoggingDomainEventPublisher` と同じ `domain_event BookingConfirmed {...}` の 1 行を標準出力に書き、同じプロセスの購読側（entitlement）に渡す）で組み立てる
- 統合テスト: `crates/interface/tests/bookings.rs`（契約の形・二重予約・体験予約の 1 回限り・確定でスロットが埋まること・403 / 404 / 409）、`crates/server/tests/bookings.rs`（モックのトークンで一周の入口を通す）、`crates/infra/tests/domain_event_payload.rs`（`BookingConfirmed` のペイロードが AsyncAPI の `required` と一致すること）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `BookingStatus` | `interface` の `contract.rs` |
| 予約 ID の UUID のテキスト形式 | `application::scheduling::booking_id_uuid` |
| 境界間イベントのペイロード（AsyncAPI） | `infra::domain_event_payload` |

## scheduling: 予約のキャンセルとキャンセル規定

- `POST /bookings/{booking_id}/cancel`（`cancelBooking`）: 生徒だけが呼べ、キャンセルできるのは自分の予約（`pending` / `confirmed`）だけ（別の生徒の予約・UUID でない ID は 404、キャンセル済みは 409 `invalid_booking_state`）。確定済みならスロットを `open` に戻す（検索に戻り、別の生徒が予約できる）。本文の `reason` は任意で、Gen1 では保存しない
- キャンセル規定（開始の何時間前以降なら回数を消費するか）は `domain::scheduling::CancellationPolicy` の 1 か所だけに置く（不変条件 3。ADR 0002 Q2）。判定の境界（ちょうどの時刻は消費する側）と前後のテストもそこにある。閾値を application・interface・infra や entitlement に写さない
- 判定の結果は境界間イベント `BookingCancelled` の `consumes_quota` に載せて発行する（scheduling → entitlement / notification / analytics）。受講権（ビデオ通話の暦月の枠）の消費の実行は entitlement が購読して行う（下の「entitlement: 月額契約の開始と通話の枠の消費」）。API のレスポンスと行動イベントには判定を載せない
- 行動イベント `booking.cancelled` を 1 回発火する。`hours_before_start` はキャンセルした時刻から開始時刻までの時間を時間単位で切り捨てた整数（開始後のキャンセルは負）
- 統合テスト: `crates/interface/tests/bookings.rs`（契約の形・23 / 25 時間前の `consumes_quota`・スロットが空くこと・403 / 404 / 409）、`crates/server/tests/bookings.rs`（モックのトークンで通す）、`crates/infra/tests/domain_event_payload.rs`（`BookingCancelled` のペイロードが AsyncAPI の `required` と一致すること）

## scheduling: スポーツごとの予約枠の需給

- `GET /slot-supply`（admin-web の `getSlotSupply`）: 本部だけが呼べる（`require_role(&[Role::Headquarters])`。コーチ・生徒は 403）。コーチの採用計画に使う。スポーツ 6 種類すべてについて、公開した枠の数 `published_slots`・予約しようとした生徒の数 `booking_students` と、足りているか `status`（`sufficient` / `insufficient`）を返す。行動イベントは発火しない
- 判定の式と期間の正本は契約（`specs/contracts/api/admin-web/openapi.yaml` の `getSlotSupply`）。実装は `domain::scheduling::SlotSupply` の 1 か所だけ（期間 `WINDOW` = 30 日、閾値 `SHORTAGE_PERCENT` = 60）で、数え方（`tally`）も domain が持つ。保存先は期間に開始する枠とその予約を読むだけ（`BookingRepository::slots_starting_between`）で、数えない。閾値と期間を application・interface・infra に写さない
- 期間は現在時刻の 30 日前（含む）から現在時刻（含まない）に開始する枠。`published_slots` は `open` / `booked` の枠、`booking_students` は（閉じた枠を含む）その枠に予約を作った生徒（予約の状態を問わず、同じ生徒は 1 人）。生徒が 1 人以上で、`booking_students >= published_slots × 0.6` なら `insufficient`
- スロットの検索は記録していない（行動イベントにも DB にもない）ので、需要は予約の作成だけから数える
- テスト: `domain::scheduling::slot_supply`（閾値の境界・0 の扱い・数え方）、`application::scheduling::get_slot_supply`（期間の端）、`crates/interface/tests/slot_supply.rs`（契約の形・期間の外を数えないこと・403 / 401）、`crates/server/tests/slot_supply.rs`（モックのトークンで通す）、`crates/infra/tests/pg_booking_repository.rs`（Postgres。期間の枠と予約の読み取り。`--include-ignored`）、`crates/seed/tests/slot_supply.rs`（合成シードの種 42 で `insufficient` のスポーツがあり、すべてではないこと。Postgres。`cargo test -p seed -- --include-ignored`）

## scheduling: 本部が見る予約の一覧（D120）

- `GET /bookings`（admin-web の `listBookings`）: 本部だけが呼べる（`interface::headquarters_bookings_router`。`require_role(&[Role::Headquarters])`。コーチ・生徒は 403）。同じパスの `POST /bookings`（student-app の予約の作成。生徒だけ）とは `merge` で 1 つのパスにまとまる。すべてのコーチの枠への予約を、状態を問わず開始の古い順（同じ開始は予約の作成の新しい順、次に `booking_id`）に返し、各行に予約（`Booking`。体験予約か・予約した時点のレッスンメニューを含む）とコーチ・生徒の表示名（profile から。引けなければ載せない）を載せる。行動イベントは発火しない
- 期間は日付（日本時間）の `from` から `to` まで（両端を含む）。省いた `from` は今日、省いた `to` は `from` から 7 日目。期間の規則（1 日以上 31 日まで）は `domain::scheduling::BookingPeriod` の 1 か所、日付と時刻の変換は `application::scheduling::calendar_day`（タイムゾーンは暦月と同じ `entitlement::calendar_month` の日本時間）。違反と `YYYY-MM-DD` でない値は 422 `validation_failed`。予約がなければ空の配列（404 はない）
- 保存先は予約枠の需給と同じ `BookingRepository::slots_starting_between` を読むだけで、DB の変更はない
- テスト: `domain::scheduling::booking_period`（1 日・31 日・逆転・超過・端）、`application::scheduling::list_bookings`（日本時間の日付の境目・既定の 7 日・並べ方・違反・0 件で表示名を問い合わせない）、`crates/interface/tests/headquarters_bookings.rs`（契約の形・既定の期間・403 / 401・422）、`crates/server/tests/bookings.rs`（`POST /bookings` と同じパスで通す）、`crates/server/tests/headquarters_bookings.rs`（Postgres の組み立て。`--include-ignored`）

## scheduling: コーチの受付の停止・再開（D116）

- `POST /coaches/{coach_id}/suspend`（admin-web の `suspendCoach`）・`POST /coaches/{coach_id}/resume`（`resumeCoach`）: 本部だけが呼べる（`require_role(&[Role::Headquarters])`。コーチ・生徒は 403）。コーチの受付（glossary の coach_acceptance。受付中・停止中）を止める・再開し、`{ coach_id, acceptance_status }` を返す。コーチでない・ない・UUID でない ID は 404、停止中の停止は 409 `coach_already_suspended`、受付中の再開は 409 `coach_not_suspended`。行動イベント・境界間イベントは発火しない
- 受付の状態の持ち主は scheduling。規則は domain の `CoachAcceptance`（`suspend`・`resume`・`ensure_accepting_bookings`）の 1 か所で、ユースケース `SuspendCoach`・`ResumeCoach` はコーチがいるかを profile に問い合わせ（`CoachIntroductionLookup`）、保存先 `CoachAcceptanceRepository::update` に判定を渡すだけ
- 停止中のコーチは: 枠の検索から除く（`SlotRepository::search`。Postgres は `NOT EXISTS` で除いた後に `LIMIT`）、予約の作成は枠の状態の判定の後で 409 `coach_not_accepting_bookings`（`CreateBooking` が `BookingRepository::find_coach_acceptance` で読む）、コーチを探す一覧（matching）から除く（`SuspendedCoachLookup`）。本部のコーチの一覧（profile の `ListCoaches`）は 1 件ごとに状態を添える（`CoachAcceptanceLookup`）。入っている予約の確定・キャンセルと、コーチの枠の公開・取り下げは止めない
- 受付の状態は予約を保存するトランザクションの前に読むので、止めた直後の並行な予約が 1 件通ることはある（止めた後の予約を止めるのが目的で、厳密な排他はしない）
- DB は `scheduling.coach_acceptances`（マイグレーション `20261031000006_expand_coach_acceptances.sql`。コーチ 1 人に 0 か 1 行、行がなければ受付中）。Postgres の停止・再開はコーチごとの advisory lock の中で読み、`INSERT ... ON CONFLICT` で書く。インメモリの `InMemoryCoachAcceptanceRepository` はスロットの保存先と共有する（`InMemorySlotRepository::coach_acceptances`）
- テスト: `domain::scheduling::coach_acceptance`、`application::scheduling`（`suspend_coach`・`resume_coach`・`scheduling_coach_acceptance_lookup`・`search_slots`・`create_booking`）・`matching::list_coaches_for_student`・`profile::list_coaches`、`crates/infra/tests/pg_coach_acceptance_repository.rs`（往復・並行な停止・検索から除く）・`schema.rs`（CHECK と外部キー）、`crates/interface/tests/coach_acceptances.rs`（200・404・409・403・401）・`coaches.rs`・`bookings.rs`、`crates/server/tests/coach_acceptance.rs`（Postgres の組み立てで一周。`--include-ignored`）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `CoachAcceptanceStatus` | `interface` の `contract.rs` |
| DB の `scheduling.coach_acceptances.status` の値 | `infra::scheduling_values`（CHECK との一致は `crates/infra/tests/schema.rs`） |

## entitlement: 契約種別と利用権の型

- 契約種別は `domain::entitlement::ContractType`（`Monthly` / `OneShot`）。Gen1 で実装するのは月額だけだが、ワンショットも型として持つ。どの契約種別でどのスポーツを結べるか（月額はゴルフ以外、ワンショットはゴルフだけ）は `ContractType::covers` の 1 か所に置き、`Entitlement::monthly` / `Entitlement::one_shot`・`Subscription::new` は破れば `EntitlementError::SportNotCoveredByContract` を返す。受講権と契約の不変条件の違反は `domain::entitlement::EntitlementError`（契約種別とスポーツ・暦月・通話の枠）で、集約・値オブジェクト・ユースケース（`StartSubscriptionError::Invalid`・`ConsumeLiveLessonQuotaError::Invalid`・`EntitlementRepository::update_by_student` のクロージャ）はこれを返す。予約の確定で `QuotaGate` が否と答えたときの拒否は scheduling の失敗として `SchedulingError::LiveLessonQuotaExceeded`（応答は同じ 409 `live_lesson_quota_exceeded`）
- 受講権 `Entitlement`: 月額は元の契約（`SubscriptionId`）・動画とチャットの送り放題・ビデオ通話の暦月の枠 `LiveLessonQuota`（契約プランと `month` と、その月に枠を使った予約の集合。`used` はその数）を持つ。ワンショットは残りの動画添削回数を持ち、通話の枠はない
- 契約プラン `SubscriptionPlan`（変更シナリオ cs-003。glossary の `subscription_plan`）: 月額契約 `Subscription` と通話の枠 `LiveLessonQuota` が持つ。`Standard`（既定）と `Plus`。暦月の上限（standard 2 回・plus 4 回。不変条件 1）は `SubscriptionPlan::live_lessons_per_month` の 1 か所だけで、`LiveLessonQuota::limit` はそこから受け取る。月をまたいでもプランは変わらない。料金・決済・契約の途中でのプランの変更は扱わない
- 月をまたいだら `used` は 0 から数える（`LiveLessonQuota::in_month` / `Entitlement::in_month`）。時刻がどの暦月かは `application::entitlement::calendar_month` の 1 か所で決める。暦月は日本時間（UTC+9）で区切る（ADR 0005）
- `GET /entitlements/me`（student-app の `getMyEntitlement`）: 生徒だけが呼べる（コーチ・本部は 403）。照会するのはトークンのアカウントの受講権で、通話の枠は今の暦月のもの。受講権がなければ 404。照会だけで何も保存しない
- ポート `EntitlementRepository`（受講権と月額契約）。`server` は当面インメモリ（`InMemoryEntitlementRepository`）。Postgres の実装はアカウントの保存先を Postgres にするときに足す（枠を予約ごとに持つので、`live_lesson_quotas` に予約ごとの行を expand で足す。ADR 0005）
- 統合テスト: `crates/interface/tests/entitlements.rs`（契約の形・月をまたいだ枠のリセット・ワンショットの形・403 / 404）、`crates/server/tests/entitlements.rs`（モックのトークンで通す）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `ContractType`（月額は `subscription`）・`SubscriptionPlan`（`standard` / `plus`。省けば standard）・`LiveLessonQuota.month`（`YYYY-MM`） | `interface` の `contract.rs` |
| DB の `entitlement.subscriptions.plan` | `infra` の `entitlement_values.rs` |
| 受講権 ID の UUID のテキスト形式 | `application::entitlement::entitlement_id_uuid` |
| 月額契約 ID の UUID のテキスト形式 | `application::entitlement::subscription_id_uuid` |
| 契約の `SubscriptionStatus` | `interface` の `contract.rs` |

## entitlement: 月額契約の開始と通話の枠の消費（不変条件 1）

いつ・どの予約で枠を消費するか、暦月の区切りは [ADR 0005](../../docs/adr/0005-live-lesson-quota-consumption.md)。

- `POST /subscriptions`（student-app の `startSubscription`）: 生徒だけが呼べる（コーチ・本部は 403）。トークンのアカウントの月額契約（`active`）と、それによる受講権（今の暦月のまだ使っていない、プランの上限の枠）を作る（`application::entitlement::StartSubscription`）。任意の `plan`（`standard` / `plus`）で契約プランを選び、省けば standard。受講権がすでにあれば 409 `subscription_already_active`、ゴルフと未知のプランは 422。境界間イベント `SubscriptionStarted` を発行し、行動イベント `subscription.started` を 1 回発火する。決済（billing）は Gen1 の範囲外で、`payment_method_token` は受け取るだけで使わない
- 枠の消費は entitlement が境界間イベントを購読して行う（`LiveLessonQuotaSubscriber`）。`BookingConfirmed` で `ConsumeLiveLessonQuota`、`BookingCancelled` の `consumes_quota: true` でも `ConsumeLiveLessonQuota`（確定で使った予約ならそのまま）、`false` なら `ReleaseLiveLessonQuota`（確定で使った分を戻す）。同じ予約で 2 回は使わない
- 上限を超える回（standard の 3 回目・plus の 5 回目）の拒否: `ConfirmBooking` は確定の前に scheduling のポート `QuotaGate` で entitlement に「もう 1 回予約してよいか」を問い合わせ（実装は `EntitlementQuotaGate`。規則は `Entitlement::may_book_live_lesson`）、否なら 409 `live_lesson_quota_exceeded`（確定しない）。scheduling は回数も上限も知らない。受講権のない生徒（月額契約の前の体験予約など）は制約を受けない
- 購読のつなぎ込み: `server` は境界間イベントを infra の `InMemoryDomainEventBus`（メッセージ基盤のモック。標準出力に 1 行書き、同じプロセスの購読側にその場で渡す。購読側の失敗は標準エラー出力に書いて捨てる）で発行する。現在時刻は `server::router_at(clock)` で差し替えられる（`router()` は OS の時計）
- Postgres では契約プランを `entitlement.subscriptions.plan`（`20261028000005_expand_subscription_plan.sql`。既存の契約は standard）に持ち、受講権を読むときに元の契約から引く。`live_lesson_quotas.remaining_live_lessons` の CHECK は 0 以上だけで、上限の数は DB に書かない
- 統合テスト: `crates/server/tests/live_lesson_quota.rs`（一周: standard は同じ暦月の 3 回目の確定が 409、翌月は再び確定できる、plus は 4 回目まで確定でき 5 回目が 409、23 時間前のキャンセルで枠が減り 25 時間前のキャンセルで戻る）、`crates/server/tests/subscription_plan.rs`（Postgres の組み立てで plus の 5 回目と standard の 3 回目が 409。`--include-ignored`）、`crates/interface/tests/subscriptions.rs`（契約の形・プラン・409・422・403 / 401）、`crates/interface/tests/bookings.rs`（409 `live_lesson_quota_exceeded`）、`crates/infra/tests/in_memory_domain_event_bus.rs`（購読側への配送）、`crates/infra/tests/domain_event_payload.rs`（`SubscriptionStarted` のペイロード）

## live-lesson: 通話セッションの開始・終了

- `POST /live-lesson-sessions`（student-app の `startLiveLessonSession`）: 予約の参加者（生徒・コーチ）だけが呼べる（役割では絞らない。参加者でないアカウントには予約がないのと同じ 404）。予約は scheduling の保存先から読み（ポート `live_lesson::BookingLookup`、実装は `scheduling::SchedulingBookingLookup`）、通話基盤にルームを作らせて参加用の `room_token` を返す。開始すると行動イベント `call.started` を 1 回発火する
- `POST /live-lesson-sessions/{session_id}/end`（`endLiveLessonSession`）: 参加者が終了する。通話基盤がルームを閉じて接続品質の生の指標（`packet_loss_rate`（百分率 0〜100）・`jitter_ms`・`round_trip_ms`・`disconnect_count`）を返し、セッションに記録して行動イベント `call.ended`（4 属性と `duration_sec`）を 1 回発火する。指標と通話品質スコアはレスポンスに載せない。スコアの式はセマンティック層の 1 か所で、バックエンドには置かない（ADR 0002 Q1）
- 不変条件は `domain::live_lesson::LiveLessonSession` が持つ: 確定した予約にしか開始できない（確定前・キャンセル済みは 409 `invalid_booking_state`）、同じ予約のセッションは 1 つ（終了済みを含め 2 回目は 409 `invalid_session_state`）、終了は開始の後（同じ時刻は許す）、終了は 1 回（409 `invalid_session_state`）。並行な開始は保存先が予約ごとに 1 つに絞る（DB では `sessions_booking_id_key`）
- 通話基盤はポート `live_lesson::VideoCallProvider`。`server` は既定で infra の `MockVideoCallProvider` で組み立てる（もう 1 つのモックへの切り替えは次の項）。Blueprint の「回線品質の低いセッションを 15% 混ぜる」に合わせ、15% の確率で品質の低い回線（損失 2〜12%・ジッタ 40〜150 ms・往復 200〜600 ms・切断 0〜3 回）、それ以外は良好な回線（損失 0〜1%・ジッタ 30 ms 以内・往復 150 ms 以内・切断なし）の一様分布から引く。テストは種を固定できる（`MockVideoCallProvider::with_seed`）。録画と実際の Twilio 連携は扱わない
- 通話基盤の切り替え（変更シナリオ cs-007）: `server` の起動時に環境変数 `VIDEO_CALL_PROVIDER` で選ぶ（`infra::VideoCallBackend`。`server::VideoCallBackend` でも公開）。知らない値（空文字列・大文字を含む）では起動しない。ポート `VideoCallProvider` の形・API の契約・行動イベント・通話品質スコアの式は、どちらを選んでも変わらない

  | `VIDEO_CALL_PROVIDER` | 通話基盤 |
  |---|---|
  | なし・`twilio` | Twilio Video 相当のモック（`MockVideoCallProvider`）。基盤の生の値がポートの単位（損失は百分率、ジッタと往復の遅延はミリ秒） |
  | `alt` | 別の通話基盤を想定したモック（`MockAltVideoCallProvider`）。基盤の生の値は損失が 0〜1 の割合、ジッタと往復の遅延が秒で、アダプタの中でポートの単位に換算する（割合 × 100、秒 × 1000 をミリ秒に四捨五入）。範囲外・負・NaN の生の値は障害（`Unavailable`）。分布は `twilio` と同じ（15% が品質の低い回線） |

  ```sh
  VIDEO_CALL_PROVIDER=alt cargo run -p server
  ```

  組み立ては `server::router_with_video_call`（`main` が使う）。`router_at` / `router_with_analytics` / `router_with_push_sender` / `router_with_postgres` は `twilio` のまま
- 保存先はポート `LiveLessonSessionRepository`。`server` は当面インメモリ（`InMemoryLiveLessonSessionRepository`）。Postgres のテーブル `live_lesson.sessions`（expand）は置いたが、実装は予約の保存先を Postgres にするときに足す。予約は scheduling の持ち物なので外部キーは張らない（ADR 0004 の 3）
- 統合テスト: `crates/interface/tests/live_lesson_sessions.rs`（契約の形・確定前の予約の 409・終了で 4 属性が `call.ended` に載ること・参加者以外の 404・422 / 401）、`crates/server/tests/live_lesson_sessions.rs`（モックのトークンと通話基盤のモックで一周）、`crates/infra/tests/mock_video_call_provider.rs`（低品質が 15% 前後。種ごとに ±0.03 の幅で見る）、`crates/infra/tests/mock_alt_video_call_provider.rs`（別の基盤の生の値からの換算を境界の値で・範囲外は障害・分布）、`crates/infra/tests/video_call_provider_contract.rs`（ポートの契約を両方のモックと `VideoCallBackend` が満たすこと）、`crates/server/tests/video_call_backend.rs`（環境変数での切り替えと、選んだ基盤で組み立てたルーターで通話を一周。`room_token` の形で基盤を見分ける）、`crates/infra/tests/schema.rs`（`live_lesson.sessions` の制約。`--include-ignored`）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `LiveLessonSessionStatus` | `interface` の `contract.rs` |
| 通話セッション ID の UUID のテキスト形式 | `application::live_lesson::session_id_uuid` |
| モックの `room_token`（`mock-room.<セッション ID>.<参加者 ID>`） | `infra` の `MockVideoCallProvider` |
| 別の基盤のモックの `room_token`（`mock-alt-room.<セッション ID>.<参加者 ID>`） | `infra` の `MockAltVideoCallProvider` |
| `VIDEO_CALL_PROVIDER` の値（`twilio` / `alt`） | `infra` の `VideoCallBackend` |

## async-lesson: 動画の送付と添削の返却

- `POST /videos`（student-app の `submitVideo`）: 生徒だけが呼べる（コーチ・本部は 403）。送る生徒はトークンのアカウント。動画の保存先（ポート `async_lesson::VideoStorage`）に本体の PUT 先を発行させ、`upload_url` で返す。動画本体はクライアントが保存先に直接 PUT し、バックエンドは通さない。送付すると境界間イベント `VideoReviewRequested` を発行し（messaging がその生徒とコーチのスレッドに、なければ開いて添削依頼のメッセージを足す。docs/adr/0007 の決定 7）、行動イベント `video.submitted` を 1 回発火する。送付の回数は数えない（月額は送り放題）。ゴルフ（golf-app の範囲）・`content_type` が `video/mp4` / `video/quicktime` でない・`coach_id` が UUID でない・`note` が 1000 文字を超えるときは 422。Gen1 はアップロードの完了を受け取らないので、レスポンスの `status` は常に `awaiting_upload`。`note` は長さだけを見て保存しない（AI の一次フィードバックの材料に渡すだけ）
- `GET /videos/{video_id}/review`（`getVideoReview`）: 動画の当事者（送った生徒・送り先のコーチ）だけが読める（役割では絞らない。当事者でないアカウント・ない動画・UUID でない ID には 404）。コーチが返すまでは `status: in_progress`（未返却）で、`review_id`・`comment`・`returned_at` は null
- `POST /videos/{video_id}/review`（admin-web の `returnVideoReview`）: コーチだけが呼べ（生徒・本部は 403）、返せるのは自分に送られた動画だけ（それ以外は 404）。返却は 1 回（2 回目は 409 `invalid_video_state`）、コメントが空か 4000 文字を超えると 422。返却すると境界間イベント `VideoReviewReturned` を発行し（notification が生徒に通知し、messaging がスレッドに添削の返却のメッセージを足す）、行動イベント `video_review.returned` を 1 回発火する。返却のリードタイムをセマンティック層で `occurred_at - submitted_at` と計算できるよう、送付の時刻 `submitted_at` も載せる
- `GET /videos/me`（student-app の `listMyVideos`。D74）: 生徒だけが呼べ（コーチ・本部は 403）、トークンの生徒が送った動画だけを、添削待ちも返却済みも含めて送った時刻の新しい順（同じ時刻は動画 ID の順）に返す。ユースケースは `ListMyVideos`（保存先の `VideoRepository::videos_of_student`）で、送り先のコーチの表示名はポート `async_lesson::VideoCoachDisplayNameLookup`（実装は `profile::ProfileCoachDisplayNameLookup`）で添える。添削の状態は `review_status`（`contract.rs` の `video_review_status_to_contract`）。添削のコメント・AI の一次フィードバックは載せず、ゴルフの動画（golf-app の範囲）は interface で除く。`router_with` の引数を増やさないよう別のルーター `interface::my_videos_router` にし、`server` が `merge` する。統合テストは `crates/interface/tests/my_videos.rs`
- 不変条件は `domain::async_lesson::Video` が持つ: 返せるのは送り先のコーチだけ、返却は 1 回、返却は送付の後（同じ時刻は許す）、着手できない動画は返せない。状態（`Submitted` / `Reviewed`）は添削の有無で決まる
- 不変条件 2（ゴルフの添削はワンショット決済が確定した動画にしか着手できない）: 規則は `domain::entitlement::ContractType::may_start_review` の 1 か所（月額は常に可、ワンショットは決済が確定した動画だけ）。返却の前にポート `async_lesson::ReviewGate` で問い合わせ、実装の `entitlement::EntitlementReviewGate` は動画のスポーツの契約種別（`ContractType::for_sport`）で答える。Gen1 はワンショットの決済（billing）を実装しないので、ゴルフの動画には常に 409 `review_payment_not_confirmed`。Gen1 は着手と返却を分けないので、返却が着手を兼ねる
- 保存先はポート `VideoRepository`。`server` は当面インメモリ（`InMemoryVideoRepository`）で、動画の保存先は infra の `MockVideoStorage`（`https://mock-storage.invalid/videos/<動画 ID>` を返す。署名・有効期限はなく、PUT しても何も保存されない）。Postgres のテーブル `async_lesson.videos` / `video_reviews`（expand）は置いたが、実装はアカウントの保存先を Postgres にするときに足す
- AI の一次フィードバック（変更シナリオ cs-006。glossary の `ai_feedback`）: `SubmitVideo` は動画を保存した直後に同じ処理の中でポート `async_lesson::AiFeedbackGenerator` を呼び（材料は `AiFeedbackRequest`: 動画の ID・スポーツ・形式・生徒の一言 `note`）、返った文と作った時刻（`Clock` の今）を `domain::async_lesson::AiFeedback` として動画に付けて保存する（`Video::record_ai_feedback`。1 本に 1 つ、送付の後）。AI・保存先のどちらが失敗しても送付は成功させ、一次フィードバックは空のまま（失敗の理由は `SubmittedVideo::ai_feedback_failure` で返し、interface が標準エラー出力に書く。application は書かない）。コーチの添削とは別の項目で、返却で上書きされず、添削の着手条件（`ReviewGate`）にも関わらない。`getVideoReview`・`returnVideoReview`・`listMyCoachVideos` の応答の `ai_feedback`（`comment`・`created_at`。なければ null）で返す。Gen1 の実装は infra の `MockAiFeedbackGenerator`（スポーツごとの決まった文。外に出ない）。Postgres は `async_lesson.videos` の `ai_feedback_comment` / `ai_feedback_created_at`（`20261028000004_expand_video_ai_feedback.sql`）。テスト: `crates/application/src/async_lesson/submit_video.rs`（成功・AI の失敗で送付は成功）、`crates/infra/tests/mock_ai_feedback_generator.rs`、`crates/infra/tests/pg_video_repository.rs`・`schema.rs`（Postgres）、`crates/interface/tests/videos.rs`（契約の形）、`crates/server/tests/ai_feedback.rs`（Postgres の組み立てで一周。`--include-ignored`）
- 送付で選ぶレッスンメニュー（D67）: `submitVideo` の任意の `lesson_menu_id` を `SubmitVideo` がポート `async_lesson::VideoLessonMenuLookup` で引き、`Video::submit_with_lesson_menu` が選べるか（送り先のコーチの・公開中の・非同期の・動画と同じスポーツのメニュー）を確かめて、送った時点の ID・名前を `domain::async_lesson::VideoLessonMenu` として動画に写す（後でメニューが直されても変わらない）。選べない・メニューがないときは何も保存せずイベントも発火せず 422 `validation_failed`。`submitVideo`・`getVideoReview`・`returnVideoReview`・`listMyCoachVideos` の応答の `lesson_menu`（`lesson_menu_id`・`name`。なければ null）で返す。Postgres は `async_lesson.videos` の `lesson_menu_id` / `lesson_menu_name`（`20261031000003_expand_video_lesson_menu.sql`。外部キーは張らない）。テスト: `crates/domain/src/async_lesson/video.rs`・`video_lesson_menu.rs`、`crates/application/src/async_lesson/submit_video.rs`、`crates/infra/tests/pg_video_repository.rs`・`schema.rs`（Postgres）、`crates/interface/tests/videos.rs`（両アプリの契約の形）
- 送り先のコーチがいるか・生徒のスポーツと合うかは確かめない（アカウントを読むポートがまだない。Postgres の実装では `videos.coach_id` の外部キーが `identity.coaches` を指す）。契約の `submitVideo` の 404 はそのときに返す
- 統合テスト: `crates/interface/tests/videos.rs`（契約の形・送付 → 返却の一連と返却前の未返却・`video.submitted` / `video_review.returned` の属性・ゴルフの 409・403 / 404 / 409 / 422 / 401）、`crates/server/tests/videos.rs`（モックのトークンと保存先のモックで一周）、`crates/infra/tests/mock_video_storage.rs`、`crates/infra/tests/schema.rs`（`async_lesson` の制約。`--include-ignored`）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `content_type`・`VideoReviewStatus`・送付のレスポンスの `VideoStatus`（`awaiting_upload`） | `interface` の `contract.rs` |
| 動画 ID・添削 ID の UUID のテキスト形式 | `application::async_lesson::video_id_uuid`・`video_review_id_uuid` |
| モックのアップロード URL | `infra` の `MockVideoStorage` |
| モックの AI の一次フィードバックの文（スポーツごと） | `infra` の `MockAiFeedbackGenerator` |

## レッスンの完了記録（is_first_lesson と LessonCompleted）

置き場所の判断（集約の外に lesson のコンテキストを作らない・テーブルは元の集約のスキーマ）と、glossary の `lesson.context` を見直す Question は [ADR 0006](../../docs/adr/0006-lesson-completion-placement.md)。

- `POST /lessons/completions`（student-app の `recordLessonCompletion`）: 同期レッスンは `{"lesson_type": "live_lesson", "session_id": ...}`、非同期レッスンは `{"lesson_type": "async_lesson", "video_id": ...}`。記録できるのは元の通話セッション・動画の当事者（生徒・コーチ）だけで、役割では絞らない（それ以外のアカウント・ない ID は 404）。ID が UUID でない・判別子に合わない ID・知らない項目は 422。終了していないセッションは 409 `invalid_session_state`、添削が返されていない動画は 409 `invalid_video_state`、同じセッション・動画の 2 回目は 409 `lesson_already_completed`
- 完了記録は元の集約が発行する: `domain::live_lesson::LiveLessonSession::complete` と `domain::async_lesson::Video::complete` が `domain::lesson::LessonCompletion` を返す。`is_first_lesson` はその生徒の完了記録が同期・非同期あわせて 0 件のときだけ true で、判定は `LessonCompletion` の 1 か所
- `application::lesson::RecordLessonCompletion` は元の集約を読み、保存先（ポート `LessonCompletionRepository::record`。同じ生徒の既存の記録を渡し、読み取りから保存までを生徒ごとに直列にする）に保存してから、行動イベント `lesson.completed`（`lesson_type`・`is_first_lesson`）を 1 回発火し、境界間イベント `LessonCompleted`（billing・review・analytics 向け）を 1 回発行する。billing・review の購読はまだない
- 保存先は当面インメモリ（`InMemoryLessonCompletionRepository`）で、通話セッション・動画の保存先と同じ中身を読む。Postgres のテーブル `live_lesson.lesson_completions` / `async_lesson.lesson_completions`（expand）は置いたが、実装は通話セッション・動画の保存先を Postgres にするときに足す（2 つのテーブルをまたいだ初回の重なりは生徒ごとの advisory lock で防ぐ）
- `GET /lessons/completions/me`（student-app の `listMyLessonCompletions`。D104）: 生徒だけが呼べ（コーチ・本部は 403）、トークンの生徒の完了したレッスンを通話・動画添削の両方とも完了した時刻の新しい順（同じ時刻は完了記録の ID の順）に返す。ページングと件数の上限はない。ユースケースは `application::lesson::ListMyLessonCompletions`（保存先の `LessonCompletionRepository::completions_of_student`）で、ほかのコンテキストの保存先は直接読まない: コーチの表示名はポート `lesson::LessonCompletionCoachDisplayNameLookup`（実装は `profile::ProfileCoachDisplayNameLookup`）、評価を付けたかはポート `lesson::LessonCompletionReviewLookup`（実装は `review::ReviewLessonCompletionReviewLookup`）、レッスンメニューは完了記録の元（通話セッションに結び付いたその生徒の予約・動画）が持つ写し（読み方は評価の一覧の `LessonCompletionMenuLookup` と共有の `lesson_completion_menus.rs`。予約は `SchedulingBookingLookup`）。行動イベントは発火しない。`router_with` の引数を増やさないよう別のルーター `interface::my_lesson_completions_router` にし、`server` が `merge` する。統合テストは `crates/interface/tests/my_lesson_completions.rs`（契約の形・並び・評価済みの印・403 / 401）と `crates/server/tests/lessons.rs` の一周の最後
- 統合テスト: `crates/interface/tests/lessons.rs`（契約の形・通話と動画の両方からの完了・同じ生徒の 2 回目で `is_first_lesson` が false・イベント・409 / 404 / 422 / 401）、`crates/server/tests/lessons.rs`（予約 → 通話 → 完了、動画 → 添削 → 完了の一周）、`crates/infra/tests/domain_event_payload.rs`（`LessonCompleted` のペイロード）、`crates/infra/tests/schema.rs`（`lesson_completions` の制約。`--include-ignored`）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `LessonType`（`live_lesson` / `async_lesson`） | `interface` の `contract.rs` |
| 完了記録 ID の UUID のテキスト形式 | `application::lesson::lesson_completion_id_uuid` |

## notification: 予約の確定・添削の返却・メッセージの受信でのプッシュ通知（モック）

- 通知のきっかけは境界間イベントの購読だけで、他のコンテキストは notification を直接呼ばない（`specs/context-map.yaml` の向き）。`application::notification::PushNotificationSubscriber` が次の 3 つを購読し、`domain::notification::PushNotification` を送る（受け取る人は `recipient()`）。ほかのイベントでは送らない（キャンセルの通知はまだない）
  - `BookingConfirmed`（scheduling）→ 生徒に `PushNotification::BookingConfirmed`（予約の確定）、予約を受けたコーチに `PushNotification::BookingReceived`（新しい予約）の 2 件（どちらもコーチ・予約の ID）
  - `VideoReviewReturned`（async-lesson。添削の返却 `ReturnVideoReview` が発行）→ 動画を送った生徒に `PushNotification::VideoReviewReturned`（コーチ・動画・添削の ID）
  - `MessageSent`（messaging。メッセージの送信 `SendMessage` が発行）→ 受け手（送り手でない側の当事者。`sender_role` が student ならコーチ、coach なら生徒）に `PushNotification::MessageReceived`（スレッド・メッセージの ID と送り手。本文は載せない）
- 送り先の基盤はポート `PushSender`。Gen1 の実装は infra の `MockPushSender`（実際の端末には届けず、送った通知を `sent()` で読めるように記録し、`push_notification <受け取るアカウントの ID> <種類> <対象の ID>` の 1 行で標準出力に書く。種類と対象は `booking_confirmed <予約の ID>`・`booking_received <予約の ID>`・`video_review_returned <動画の ID>`・`message_received <スレッドの ID>`）。テスト用に `in_memory::RecordingPushSender`。表示する文言は扱わない（本物の基盤につなぐときに infra のアダプタで決める）
- 送る前にその通知をお知らせとして残し（下の「お知らせ」）、次に受け取る人の通知の受け取り設定（下の「通知の受け取り設定」）を読み、その通知の種類（`PushNotification::kind()`）がオフなら送らない
- 同じイベントの再送で 2 回送らないよう、送った通知を `event_id` と組にしてプロセスのメモリに覚える（受け取り設定がオフで送らなかった通知も覚え、あとでオンに戻しても再送では送らない。受け取り設定を読めなかった通知は送れなかった通知と同じく再送で送り直す。1 イベントで 2 件送る予約の確定も通知ごとに覚え、送れなかった通知だけを再送で送り直す。1 件が送れなくても残りは送り、障害を返す）。プロセスを再起動すると忘れる
- 組み立ては `server` の `InMemoryDomainEventBus` の購読側（`MessageThreadSubscriber` と組にして足した）。`ReturnVideoReview`・`SendMessage` にも同じバスを発行先として渡す。テストから送り先を差し替える組み立ては `server::router_with_push_sender`
- テスト: `crates/domain/src/notification/push_notification.rs`（受け取る人）、`crates/application/src/notification/push_notification_subscriber.rs`（予約の確定は生徒とコーチに 1 件ずつ・1 イベント 1 回・メッセージは送り手でない側へ・ほかのイベントで送らない・失敗後の再送・片方だけ失敗したときは失敗した方だけを再送で送る）、`crates/application/src/async_lesson/return_video_review.rs`・`messaging/send_message.rs`（保存したら発行し、拒否では発行しない）、`crates/infra/tests/domain_event_payload.rs`（`VideoReviewReturned` / `MessageSent` のペイロードが AsyncAPI の `required` と一致すること）、`crates/infra/tests/mock_push_sender.rs`（記録と 1 行の形）、`crates/server/tests/notifications.rs`（予約の作成では送らず、確定で生徒とコーチに 1 件ずつ、キャンセルでは送らない。動画の送付では送らず、返却で生徒に 1 件。コーチの送信は生徒へ、生徒の送信はコーチへ、拒否された送信では送らない）

### 通知の受け取り設定（D16）

- アカウントごと・通知の種類ごとに受け取るか（オン / オフ）を決める（glossary の `notification_settings`、domain の `NotificationSettings`）。既定はすべてオンで、変えたことのない種類は保存しない。種類（`NotificationKind`）は通知のきっかけごとに 3 つ: `BookingConfirmed`（予約の確定。生徒への `BookingConfirmed` とコーチへの `BookingReceived` の両方）・`VideoReviewReturned`（添削の返却）・`MessageReceived`（メッセージの受信）
- student-app の `GET /notification-settings/me`（`getMyNotificationSettings`）と `PUT /notification-settings/me`（`updateMyNotificationSettings`）。生徒だけ（ほかの役割は 403）。PUT は書いた種類だけを変え、書かなかった種類は今のまま（真偽値でなければ 422）。行動イベントは発火しない。契約の項目名（`booking_confirmed` / `video_review_returned` / `message_received`）との対応は `interface` の `notification_settings.rs` だけ
- 保存先はポート `NotificationSettingsRepository`（保存していなければ既定を返す。変更は読み取りから保存までを直列にする）。Postgres は `notification.notification_settings`（アカウント × 種類で 1 行。マイグレーション `20261028000001_expand_notification_settings.sql`、種類の文字列は `infra::notification_values`）。`PgNotificationSettingsRepository` はアカウントごとの advisory lock の中で読み、すべての種類の行を upsert する。`router_at` などの組み立ては当面インメモリ（`InMemoryNotificationSettingsRepository`）、`router_with_postgres` は Postgres
- コーチの受け取り設定を変える操作（admin-web）はまだない。コーチへの通知（`BookingReceived`・コーチへの `MessageReceived`）も同じ表を読むので、行がなければすべて送る
- テスト: `crates/domain/src/notification/notification_settings.rs`（既定はすべてオン・オフとオンの切り替え）、`crates/application/src/notification/`（`get_notification_settings.rs`・`update_notification_settings.rs`、`push_notification_subscriber.rs` のオフなら送らず再送でも送らない・オフの種類だけを止める・設定を読めなければ再送で送る）、`crates/infra/tests/pg_notification_settings_repository.rs`（既定・往復・並行な変更が互いを消さない）・`schema.rs`（種類の CHECK・1 アカウント 1 種類 1 行・アカウントへの外部キー）、`crates/interface/tests/notification_settings.rs`（契約の形・既定・部分の変更・422・403・401）、`crates/server/tests/notifications.rs`（オフにした生徒には送らずコーチには送る・Postgres の組み立てで保存される）

### お知らせ（D108）

- 送ったプッシュ通知を、受け取る人があとで一覧で見られるように残す（glossary の `notification`、domain の `Notification`。中身の `PushNotification`・作った時刻・既読の時刻）。`PushNotificationSubscriber` が送る前に残し（作った時刻は `Clock` の今）、受け取り設定でオフの種類も残す（オフが止めるのはプッシュ通知だけ）。お知らせを残せなければ送らずに障害を返し、再送でやり直す。コーチへの通知（`BookingReceived`・コーチへの `MessageReceived`）も残すが、コーチが読む API はまだない
- student-app の `GET /notifications/me`（`listMyNotifications`。作った時刻の新しい順、同じ時刻は後に残したものが先。ページングなし）と `POST /notifications/me/read`（`markMyNotificationsRead`。すべて既読にして一覧を返す）。生徒だけ（ほかの役割は 403）。各行は `notification_id`・`kind`（`NotificationKind` と同じ綴り）・`body`・`created_at`・`is_read`。本文（利用者に見せる文言）と種類の値の対応は `interface` の `notifications.rs` だけ。行動イベントは発火しない
- 既読は domain の `Notification::mark_read`（最初に読んだ時刻のまま。既読にする時刻より後に作ったものは読まない）。保存先から戻す `Notification::restore` も、既読が作った時刻より前なら `NotificationError::NotificationReadBeforeCreated` で断る（DB の `notifications_read_after_created` と同じ不変条件）。ユースケースは `ListMyNotifications`・`MarkMyNotificationsRead`
- 保存先はポート `NotificationRepository`（同じプッシュ通知のお知らせは 1 件。再送や送り直しで 2 件にしない）。Postgres は `notification.notifications`（マイグレーション `20261031000004_expand_notifications.sql`。種類の文字列は `infra::notification_values` の `push_notification_kind_value`）。`PgNotificationRepository` は `ON CONFLICT DO NOTHING` で残し（`notifications_one_per_push`）、既読は受け取る人のまだ読んでいない行を `FOR UPDATE` で読んで `mark_read` に通してから書く。`router_at` などの組み立ては当面インメモリ（`InMemoryNotificationRepository`）、`router_with_postgres` は Postgres
- テスト: `crates/domain/src/notification/notification.rs`（既読の時刻）、`crates/application/src/notification/`（`list_my_notifications.rs`・`mark_my_notifications_read.rs`、`push_notification_subscriber.rs` の送った通知を 1 件ずつ残す・オフでも残す・送り直しで 2 件にしない・残せなければ送らない）、`crates/infra/tests/pg_notification_repository.rs`（変種ごとの往復・受け取る人ごとの新しい順・同じプッシュ通知は 1 件・既読）・`schema.rs`（種類の CHECK・中身の形・1 通知 1 件・既読は作った後）、`crates/interface/tests/notifications.rs`（契約の形・自分の分だけ・新しい順・すべて既読・403・401）、`crates/server/tests/notifications.rs`（オフにした種類もお知らせに残り、既読にできる）

### 本部からのお知らせ（D124）

- 本部が生徒全員か、あるスポーツの生徒全員にお知らせを送る（glossary の `announcement`、domain の `Announcement`。題 `AnnouncementTitle`（1〜50 文字）・本文 `AnnouncementBody`（1〜2000 文字）・対象のスポーツ・受け取った生徒の数・送った時刻）。admin-web の `POST /announcements`（`sendAnnouncement`。201）と `GET /announcements`（`listAnnouncements`。送った時刻の新しい順）。本部だけ（ほかの役割は 403）。行動イベントは発火しない
- ユースケース `SendAnnouncement` は対象の生徒を profile に問い合わせ（ポート `AnnouncementRecipientLookup`、実装 `ProfileAnnouncementRecipientLookup`）、いなければ `NotificationError::AnnouncementWithoutRecipients`（404）で何も残さない。送った記録を残してから生徒ごとにお知らせ（`PushNotification::Announcement`、種類 `NotificationKind::Announcement`）を残し、受け取り設定でオンの生徒にだけプッシュ通知を送る。プッシュ通知を送れなくても止めない（お知らせは残っている）
- 生徒のお知らせの本文は送った記録の題と本文を改行でつないだもの。`ListMyNotifications`・`MarkMyNotificationsRead` は本部からのお知らせに送った記録を添えて返す（`MyNotification`）。student-app の通知の受け取り設定に `announcement` を足した
- 保存先はポート `AnnouncementRepository`。Postgres は `notification.announcements`（マイグレーション `20261101000003_expand_announcements.sql`）。同じマイグレーションで `notification.notifications` に `announcement_id`（送った記録への外部キー）を足し、`coach_id` の NOT NULL を外して種類ごとの形（`notifications_kind_shape`）で守り、同じお知らせは 1 人に 1 件（`notifications_one_per_announcement`）。`router_at` などの組み立ては当面インメモリ（`InMemoryAnnouncementRepository`）、`router_with_postgres` は Postgres
- テスト: `crates/domain/src/notification/`（`announcement.rs`・`announcement_title.rs`・`announcement_body.rs`）、`crates/application/src/notification/`（`send_announcement.rs`・`list_announcements.rs`・`list_my_notifications.rs`）と `crates/application/src/profile/profile_announcement_recipient_lookup.rs`、`crates/infra/tests/pg_announcement_repository.rs`・`pg_notification_repository.rs`・`schema.rs`、`crates/interface/tests/announcements.rs`（契約の形・全員とスポーツ・受け取り設定・一覧・404・422・403・401）・`notifications.rs`、`crates/server/tests/notifications.rs`（生徒のお知らせに届く・Postgres の組み立てで保存される）

## 行動イベントの配信先（モック）と発火の網羅

行動イベントは `specs/analytics/events.yaml` が正で、KPI の式（セマンティック層）は `specs/analytics/kpi.yaml` の 1 か所に置く。バックエンドは式を持たず、生の属性を発火するだけ。

- 発火はポート `application::analytics::AnalyticsSink`。配信先は `server` が環境変数で選ぶ（`server::AnalyticsDestination`）。知らない `ANALYTICS_SINK` の値では起動しない

  | 環境変数 | 値 |
  |---|---|
  | `ANALYTICS_SINK` | `memory`（既定。プロセスの中に記録するだけ）・`object_store`（オブジェクトストアに書く） |
  | `ANALYTICS_OBJECT_STORE_ENDPOINT` | S3 API のエンドポイント（既定 `http://127.0.0.1:9090`。compose の `objectstore`） |
  | `ANALYTICS_OBJECT_STORE_BUCKET` | バケット（既定 `rally-analytics`。compose が作る） |
  | `ANALYTICS_RELEASE_ID` | 共通の任意属性 `release_id`（版の ID。例: `guccyon/rally@0.1.0`）。ないか空なら付けない |
  | `ANALYTICS_APP_VERSION` | 共通の任意属性 `app_version`（アプリの版。例: `0.1.0`）。ないか空なら付けない |

- `object_store` は infra の `ObjectStoreAnalyticsSink`（Firebase / GA 相当のモック）。発火 1 回で 1 オブジェクト `events/<name>/dt=<UTC の日付>/<UUID>.jsonl` を書き、中身は JSON Lines の 1 行 `{"name": ..., "attributes": {...}}`（属性は events.yaml の `required` に、共通の任意属性 `common_optional`（`release_id`・`app_version`）のうち値が分かるものを足したもの。値は `ANALYTICS_RELEASE_ID` / `ANALYTICS_APP_VERSION` から読み（`infra::AnalyticsCommonAttributes`）、分からなければ付けずに発火する。対応表は `infra::analytics_event_payload`）。ローカルのファイルシステムには何も残さない。モックなので認証情報は持たず署名しない。書けなければ標準エラー出力に書いて捨て、ユースケースは失敗させない（タイムアウトと再試行を短くして、配信先が止まっていても数秒で戻る）
- compose の `objectstore` は S3 互換のモック（`adobe/s3mock`。認証なし。中身は Docker のボリューム `objectstore-data`）

```sh
docker compose up -d --wait objectstore
ANALYTICS_SINK=object_store cargo run -p server
curl -s 'http://127.0.0.1:9090/rally-analytics?list-type=2'   # 書かれたオブジェクトの一覧（S3 の ListObjectsV2）
```

| 何の文字列か | 置き場所 |
|---|---|
| 行動イベントの属性（events.yaml の `required` と `common_optional`）・`lesson_type` の値・JSON Lines の 1 行 | `infra::analytics_event_payload`（`sport` は `infra::identity_values`、時刻は `domain_event_payload` と同じ RFC 3339） |

- テスト: `crates/infra/tests/analytics_event_payload.rs`（全イベントの属性が events.yaml の `required` とちょうど一致すること・形・共通の任意属性は値が分かるときだけ付き、どのイベントの `required` にもないこと）、`crates/infra/tests/analytics_common_attributes.rs`（環境変数から共通の任意属性を読むこと）、`crates/infra/tests/object_store_analytics_sink.rs`（インメモリのストアと偽の S3 で、1 行ずつ書くこと・パス形式の PUT・止まった配信先で数秒で戻ること）、`crates/server/tests/analytics_destination.rs`（環境変数での切り替えと、`POST /accounts` で `account.registered` が偽の S3 に届く一周。版の環境変数があれば `release_id` / `app_version` が付くこと）
- 発火の網羅: `crates/application/tests/analytics_emission.rs` がソースを静的に読み、events.yaml の全イベントに発火するユースケースがあることを、テストの中の対応表（イベント名 → `AnalyticsEvent` の変種 → ユースケースとファイル）で確かめる。イベントを足したら表に 1 行足す。表にない `emit` も落とす
- セマンティック層: `crates/application/tests/kpi_semantic_layer.rs` が kpi.yaml を読み、6 つの KPI の式が events.yaml にある名前（イベント名と `required` の属性）と語彙だけを参照し、`inputs` が式の読むイベントと一致すること、events.yaml の `kpi` がその KPI の `inputs` に入っていることを確かめる。CI は `apps/backend` に変更があるときだけ走るので、specs/ だけを変えたら手元で `cargo test -p application` を走らせる

## 一周シナリオの E2E（API レベル）

Walking Skeleton の完了条件（段 7 の T19）。`crates/server/tests/walking_skeleton.rs` が、保存先をすべて Postgres にした組み立て（`server::router_with_postgres`。通話基盤・動画の保存先・アクセストークンはモック、境界間イベントはインメモリのイベントバス、行動イベントはインメモリの `RecordingAnalyticsSink`）を TCP で待ち受けさせ、HTTP だけで次を通す。

1. 生徒の登録 → コーチが枠を公開 → 生徒が検索して体験予約 → 確定
2. 通話の開始 → 終了 → 完了記録（`is_first_lesson=true`）
3. 同じ生徒が月額契約 → 動画を送付 → コーチが添削を返す → 完了記録（`is_first_lesson=false`）
4. 同じ暦月に上限を超える通話予約の確定が 409 `live_lesson_quota_exceeded`（上限は契約プランの `live_lessons_per_month`。この一周はプランを選ばない standard の契約なので、月額契約の前に確定した体験予約は枠を使わず、契約後の 2 回を確定してからの 3 回目）

最後に、発火した行動イベントの名前の並びが期待した列（events.yaml の `name`）と一致することを確かめる。コーチの登録は契約に HTTP の操作がない（`POST /accounts` は生徒だけ）ので、同じ DB に `RegisterAccount` で登録してモックのトークンを発行する。

Postgres が要るので既定では走らない（`#[ignore]`）。`DATABASE_URL` がなければ skip ではなく失敗する（テストごとに新しい DB を作ってマイグレーションを当てる）。CI に Postgres のサービスを足すまでは手元で走らせる。

```sh
export DATABASE_URL=postgres://rally@127.0.0.1:5432/rally   # docker compose up -d --wait の Postgres
cargo test -p server --test walking_skeleton -- --include-ignored
```

## CI で走らせるコマンド

`apps/backend/` で次の 3 つを順に走らせ、どれかが落ちたらマージしない（L1）。依存方向の検査は `cargo test` に含まれる（`crates/architecture/tests/dependency_direction.rs`）。

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

L2（テストの実質強度）として、PR の差分に cargo-mutants の差分ミューテーションを走らせる（`.github/workflows/mutation.yml`。shadow で PR は落とさない）。手元では `cargo install cargo-mutants` の後に `cargo mutants -p domain` などで確かめる（結果は `mutants.out/`）。等価な変異（どのテストでも振る舞いが変わらない変異）だけを `.cargo/mutants.toml` の `exclude_re` で除き、理由を書き添える。
