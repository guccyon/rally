# apps/backend

Rally のバックエンド（Rust / axum / PostgreSQL）の Cargo ワークスペース。層の規約と依存方向は [ADR 0003](../../docs/adr/0003-backend-layers.md)。

## 構成

| パス | 層 | 依存してよいクレート |
|---|---|---|
| `crates/domain/` | ドメイン。純粋（I/O なし）。`identity`（アカウント・役割・スポーツ・コーチの自己紹介文 `CoachBio` と不変条件）・`scheduling`（スロットと、重なりを判定する集約 `CoachCalendar`・予約 `Booking`・キャンセル規定 `CancellationPolicy`・スポーツごとの予約枠の需給 `SlotSupply`）・`entitlement`（契約種別 `ContractType`・受講権 `Entitlement`・通話の暦月の枠 `LiveLessonQuota`・暦月 `YearMonth`・月額契約 `Subscription`）・`live_lesson`（通話セッション `LiveLessonSession`・接続品質の生の指標 `ConnectionQuality`）・`async_lesson`（動画 `Video`・添削 `VideoReview`・添削コメント `ReviewComment`・動画の形式 `VideoContentType`）・`lesson`（live-lesson と async-lesson が共有するレッスンの完了記録 `LessonCompletion`・`LessonId`・`LessonType`・`LessonSource`。コンテキストではない）・`notification`（プッシュ通知 `PushNotification`・通知の種類 `NotificationKind`・通知の受け取り設定 `NotificationSettings`） | なし |
| `crates/application/` | ユースケースとポート（`identity::RegisterAccount`・`ListCoaches`・`GetAccount`・`UpdateCoachProfile`・`AccountRepository`・`TokenIssuer`・`TokenVerifier`・`scheduling::PublishSlot`・`SearchSlots`・`SlotRepository`・`CoachIntroductionLookup`（スロットの検索に添えるコーチの紹介。実装は `identity::IdentityCoachIntroductionLookup`）・`CreateBooking`・`ConfirmBooking`・`CancelBooking`・`GetSlotSupply`・`ListMyBookings`（自分の予約の一覧）・`BookingRepository`・`QuotaGate`・`entitlement::GetEntitlement`・`StartSubscription`・`ConsumeLiveLessonQuota`・`ReleaseLiveLessonQuota`・`LiveLessonQuotaSubscriber`・`EntitlementQuotaGate`・`EntitlementRepository`・`live_lesson::StartLiveLessonSession`・`EndLiveLessonSession`・`LiveLessonSessionRepository`・`VideoCallProvider`・`BookingLookup`（実装は `scheduling::SchedulingBookingLookup`）・`async_lesson::SubmitVideo`・`GetVideoReview`・`ReturnVideoReview`・`VideoRepository`・`VideoStorage`・`ReviewGate`（実装は `entitlement::EntitlementReviewGate`）・`lesson::RecordLessonCompletion`・`LessonCompletionRepository`・`notification::PushNotificationSubscriber`・`PushSender`・`GetNotificationSettings`・`UpdateNotificationSettings`・`NotificationSettingsRepository`・`AnalyticsSink`・`DomainEventPublisher`・`DomainEventSubscriber`・`Clock`）。ポートのインメモリ実装（`in_memory`）。ID と UUID の変換（`identity::account_id_uuid`・`scheduling::slot_id_uuid`・`scheduling::booking_id_uuid`・`entitlement::entitlement_id_uuid`・`entitlement::subscription_id_uuid`・`live_lesson::session_id_uuid`・`async_lesson::video_id_uuid`・`async_lesson::video_review_id_uuid`・`lesson::lesson_id_uuid`）・時刻から暦月（`entitlement::calendar_month`）・障害の型（`Unavailable`） | domain |
| `crates/infra/` | Postgres（`migrations/`・`PgSlotRepository`）・モックの外部連携（`MockTokenService`: アクセストークン、`LoggingDomainEventPublisher`: 境界間イベントを標準出力に書く、`InMemoryDomainEventBus`: 境界間イベントを同じプロセスの購読側に渡す、`MockVideoCallProvider`: 通話基盤。終了時に接続品質の指標を乱数で生成する、`MockVideoStorage`: 動画の保存先。固定の形のアップロード URL を返す、`MockPushSender`: プッシュ通知の送り先。送った通知を記録して標準出力に書く、`ObjectStoreAnalyticsSink`: 行動トラッキングの配信先。S3 API のオブジェクトストアに JSON Lines で書く）・DB とトークンと境界間イベント・行動イベントのペイロードの文字列の対応表（`identity_values`・`scheduling_values`・`domain_event_payload`・`analytics_event_payload`） | domain・application |
| `crates/interface/` | axum の HTTP（`GET /healthz`、`POST /accounts`、`GET /slots`、`POST /slots`、`POST /bookings`、`GET /bookings/me`、`POST /bookings/{booking_id}/confirm`、`POST /bookings/{booking_id}/cancel`、`GET /slot-supply`、`GET /coaches`、`GET /me`、`PUT /me/profile`、`POST /subscriptions`、`GET /entitlements/me`、`GET /notification-settings/me`、`PUT /notification-settings/me`、`POST /live-lesson-sessions`、`POST /live-lesson-sessions/{session_id}/end`、`POST /videos`、`GET /videos/{video_id}/review`、`POST /videos/{video_id}/review`、`POST /lessons/completions`）・認証（`AuthenticatedAccount`・`require_role`）・契約の文字列の対応表と API の文言 | domain・application（infra は不可） |
| `crates/server/` | 組み立て用（composition root）。interface に infra の実装を渡し、HTTP サーバーを起動する。行動イベントの配信先を環境変数で選ぶ（`AnalyticsDestination`）。保存先をすべて Postgres にした組み立ては `router_with_postgres`（一周シナリオの E2E が使う） | domain・application・infra・interface |
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
```

ポートは組み立て用のクレート `server` で組み立てる（ADR 0003 の決定 5）。アカウント・スロット・予約・受講権・通話セッション・動画の保存先は当面 `application::in_memory` のインメモリ実装なので、登録したアカウントや公開した枠はプロセスを止めると消える。Postgres の実装は infra に置き、`server::router_with_postgres` で組み立てられる（`main` はまだ使わない）。
行動イベントの配信先は環境変数 `ANALYTICS_SINK` で選ぶ（既定はインメモリ。下の「行動イベントの配信先」）。

## identity の不変条件

`domain::identity` がモデルの生成時に検証し、破れば `DomainError` を返す。

- 役割（生徒・コーチ・本部）とスポーツの組み合わせ: 生徒はちょうど 1 つ、コーチは 1 つ以上（提供スポーツの集合）、本部はなし
- スポーツは Blueprint の 5 種類（tennis / golf / baseball / soccer / basketball）のどれか。student-app の `POST /accounts` はさらにゴルフを 422 で断る（契約の `Sport` がゴルフを含まない）
- コーチの自己紹介文（`CoachBio`）は空でもよく 400 文字まで（契約の `maxLength`。前後の空白も削らずに持つ）。登録したばかりのコーチは空
- 表示名は空（空白だけ）でなく 50 文字まで（契約の `maxLength`）。メールアドレスは `@` の前後が空でない最小の形で、一意性は大文字小文字を区別しない

domain は列挙型と値（`AccountId` は `u128`）だけを持ち、外部の文字列表現を知らない。対応表と文言は境界に置く。

| 何の文字列か | 置き場所 |
|---|---|
| 契約（OpenAPI）の `Sport` の値 | `interface` の `contract.rs` |
| API の `message`（`DomainError` の変種・役割の呼び名から組み立てる） | `interface` の `message.rs` |
| DB の `identity.accounts.role` / `sport` の値・モックのトークンの役割 | `infra::identity_values`（CHECK との一致は `crates/infra/tests/schema.rs`） |
| アカウント ID の UUID のテキスト形式（契約の `format: uuid`・トークン） | `application::identity::account_id_uuid`（`uuid` クレートに任せる。interface と infra の両方から使うので application に置く） |

保存先の障害は `application::Unavailable`（原因を `source` に持つ）で返す。interface は 500 `internal` に固定の文言だけを返し、原因はクライアントに出さない（標準エラー出力に書く。統合テストは `crates/interface/tests/internal_error.rs`）。

`POST /accounts` の統合テスト（`crates/interface/tests/accounts.rs`）は、レスポンスを `specs/contracts/api/student-app/openapi.yaml` のスキーマで検査する。生徒の登録で行動イベント `account.registered` が 1 回発火することもここで確かめる。トークンは `TokenIssuer` が発行する（このテストではテスト用の偽物）。

## identity: 本部向けのコーチの一覧

- `GET /coaches`（admin-web の `listCoaches`）: 本部だけが呼べる（`require_role(&[Role::Headquarters])`。コーチ・生徒は 403）。コーチの役割のアカウントの `coach_id` と `display_name` を返す。絞り込み・ページングはせず、行動イベントは発火しない
- 並べ方の正本は契約（表示名の Unicode のコードポイントの順、同じ表示名は `coach_id` の順）。保存先のポート `AccountRepository::list_coaches` が並べて返す。Postgres は `ORDER BY display_name COLLATE "C", account_id`（DB の照合順序に依らない。UTF-8 のバイト順はコードポイントの順と同じ）、インメモリは `(表示名の文字列, AccountId)` の順で、両方が同じ並びになる。読んだ行は `Account`（提供スポーツを含む）に戻す
- テスト: `application::identity::list_coaches`（コーチだけ・並べ方）、`crates/interface/tests/coaches.rs`（契約の形・並べ方・403 / 401）、`crates/infra/tests/pg_account_repository.rs`（Postgres。照合順序に依らない並びとスポーツの復元。`--include-ignored`）

## identity: コーチの自己紹介文（D18）

- `PUT /me/profile`（admin-web の `updateMyCoachProfile`）: コーチだけが呼べる（`require_role(&[Role::Coach])`。本部・生徒は 403）。トークンのコーチの自己紹介文を `bio` で丸ごと置き換え、`GET /me` と同じ形の `Account` を返す。`bio` がない・文字列でない・400 文字を超えると 422（何も変えない）。トークンのコーチのアカウントがなければ `GET /me` と同じく 401。行動イベントは発火しない
- `GET /me`（`getMyAccount`）はコーチのときだけ `bio` を返す（書いていなければ空文字列）。本部・生徒の応答には載らない
- 自己紹介文は domain の `Coach` の属性（`CoachBio`）で、ユースケースは `application::identity::UpdateCoachProfile`。保存先はポート `AccountRepository::update_coach_bio`（コーチのアカウントがなければ何も変えずに `None`）。Postgres は `identity.coaches.bio`（マイグレーション `20261028000002_expand_coach_bio.sql`。既定は空、`coaches_bio_length` で 400 文字まで）
- テスト: `crates/domain/src/identity/coach_bio.rs`（空・上限・超過）、`application::identity::update_coach_profile`（置き換え・空に戻す・超過で変えない・コーチ以外）、`crates/infra/tests/pg_account_repository.rs`（Postgres の往復・コーチ以外は変えない）・`schema.rs`（既定の空・`coaches_bio_length`）、`crates/interface/tests/me.rs`（契約の形・コーチ / 本部 / 生徒・422・401）、`crates/server/tests/coach_profile.rs`（Postgres の組み立てで保存される。`--include-ignored`）

## 認証と認可（モック）

- アクセストークンはアカウント ID と役割を載せた不透明な文字列で、`Authorization: Bearer <access_token>` で送る（契約の `bearerAuth`）。発行・検証は application のポート `identity::TokenIssuer` / `TokenVerifier`、実装は infra の `MockTokenService` だけ
- `MockTokenService` は**署名なし**の固定形式 `mock.<役割>.<アカウント ID>` で、誰でも任意の役割のトークンを作れる。Gen1 のローカル起動とテスト用で、本番では署名付きの実装に差し替える。パスワード・外部 IdP・失効は扱わない
- `POST /accounts` と `GET /healthz` 以外のエンドポイントは認証必須にする。ハンドラーの引数に `interface::AuthenticatedAccount` を取れば、トークンがない・無効なら 401 `unauthorized`（`WWW-Authenticate: Bearer`）。役割で絞るルートは `route_layer(interface::require_role(&[Role::Coach]))` を重ね、役割が合わなければ 403 `forbidden`（admin-web の本部とコーチの同居。ADR 0002 Q3）
- 401 / 403 / 200 の統合テストは `crates/server/tests/auth.rs`（infra のモックで発行したトークンと、`POST /accounts` が返したトークンを使う）

## scheduling: スロットの公開と検索

- `POST /slots`（admin-web の `publishSlot`。`specs/contracts/api/admin-web/openapi.yaml`）: コーチだけが呼べる（`require_role(&[Role::Coach])`。生徒・本部は 403）。枠のコーチはトークンのアカウント。公開すると行動イベント `slot.published` を 1 回発火する
- `GET /slots`（student-app の `searchSlots`）: 認証した利用者なら誰でも呼べる。公開中（`open`）の枠をスポーツ（必須。ゴルフは 422）・コーチ・開始時刻の期間（`starts_after` 以上 `starts_before` 未満。`starts_after` の既定は現在時刻）で絞り、（開始時刻, スロット ID）の昇順で返す。`limit` は 1〜100（既定 20）、`next_cursor` は次のページの位置（クライアントには不透明。形は `interface` の `contract.rs`）
- 不変条件は `domain::scheduling` が持つ: 開始 < 終了（期間は終了を含まない）、Gen1 の定員は 1、同じコーチの時間を占める枠（`open` / `booked`）は重ならない（集約 `CoachCalendar`。`closed` は時間を占めない）。重なりは 409 `slot_overlaps`
- 重なりの判定は domain だけに置き、DB の排他制約は置かない。Postgres の実装（`infra::PgSlotRepository`）はコーチごとの advisory lock で並行な公開を直列にする（ADR 0004 の 4）
- `server` はスロットの保存先も当面インメモリ（`InMemorySlotRepository`）。`scheduling.slots.coach_id` が `identity.coaches` を指すので、アカウントの保存先を Postgres にするときに一緒に差し替える
- 統合テスト: `crates/interface/tests/slots.rs`（契約の形・403・絞り込み・ページング）、`crates/server/tests/slots.rs`（モックのトークンで通す）、`crates/infra/tests/pg_slot_repository.rs`（Postgres。重なりの拒否と並行な公開・検索。`--include-ignored`）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `Sport`（admin-web は 5 種類、student-app はゴルフを除く 4 種類）・`SlotStatus`・`date-time`・`next_cursor` | `interface` の `contract.rs` |
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

- `GET /slot-supply`（admin-web の `getSlotSupply`）: 本部だけが呼べる（`require_role(&[Role::Headquarters])`。コーチ・生徒は 403）。コーチの採用計画に使う。スポーツ 5 種類すべてについて、公開した枠の数 `published_slots`・予約しようとした生徒の数 `booking_students` と、足りているか `status`（`sufficient` / `insufficient`）を返す。行動イベントは発火しない
- 判定の式と期間の正本は契約（`specs/contracts/api/admin-web/openapi.yaml` の `getSlotSupply`）。実装は `domain::scheduling::SlotSupply` の 1 か所だけ（期間 `WINDOW` = 30 日、閾値 `SHORTAGE_PERCENT` = 60）で、数え方（`tally`）も domain が持つ。保存先は期間に開始する枠とその予約を読むだけ（`BookingRepository::slots_starting_between`）で、数えない。閾値と期間を application・interface・infra に写さない
- 期間は現在時刻の 30 日前（含む）から現在時刻（含まない）に開始する枠。`published_slots` は `open` / `booked` の枠、`booking_students` は（閉じた枠を含む）その枠に予約を作った生徒（予約の状態を問わず、同じ生徒は 1 人）。生徒が 1 人以上で、`booking_students >= published_slots × 0.6` なら `insufficient`
- スロットの検索は記録していない（行動イベントにも DB にもない）ので、需要は予約の作成だけから数える
- テスト: `domain::scheduling::slot_supply`（閾値の境界・0 の扱い・数え方）、`application::scheduling::get_slot_supply`（期間の端）、`crates/interface/tests/slot_supply.rs`（契約の形・期間の外を数えないこと・403 / 401）、`crates/server/tests/slot_supply.rs`（モックのトークンで通す）、`crates/infra/tests/pg_booking_repository.rs`（Postgres。期間の枠と予約の読み取り。`--include-ignored`）、`crates/seed/tests/slot_supply.rs`（合成シードの種 42 で `insufficient` のスポーツがあり、すべてではないこと。Postgres。`cargo test -p seed -- --include-ignored`）

## entitlement: 契約種別と利用権の型

- 契約種別は `domain::entitlement::ContractType`（`Monthly` / `OneShot`）。Gen1 で実装するのは月額だけだが、ワンショットも型として持つ。どの契約種別でどのスポーツを結べるか（月額はゴルフ以外、ワンショットはゴルフだけ）は `ContractType::covers` の 1 か所に置き、`Entitlement::monthly` / `Entitlement::one_shot`・`Subscription::new` は破れば `DomainError::SportNotCoveredByContract` を返す
- 受講権 `Entitlement`: 月額は元の契約（`SubscriptionId`）・動画とチャットの送り放題・ビデオ通話の暦月の枠 `LiveLessonQuota`（`month` と、その月に枠を使った予約の集合。`used` はその数。上限 2 は `LiveLessonQuota` の中の 1 か所）を持つ。ワンショットは残りの動画添削回数を持ち、通話の枠はない
- 月をまたいだら `used` は 0 から数える（`LiveLessonQuota::in_month` / `Entitlement::in_month`）。時刻がどの暦月かは `application::entitlement::calendar_month` の 1 か所で決める。暦月は日本時間（UTC+9）で区切る（ADR 0005）
- `GET /entitlements/me`（student-app の `getMyEntitlement`）: 生徒だけが呼べる（コーチ・本部は 403）。照会するのはトークンのアカウントの受講権で、通話の枠は今の暦月のもの。受講権がなければ 404。照会だけで何も保存しない
- ポート `EntitlementRepository`（受講権と月額契約）。`server` は当面インメモリ（`InMemoryEntitlementRepository`）。Postgres の実装はアカウントの保存先を Postgres にするときに足す（枠を予約ごとに持つので、`live_lesson_quotas` に予約ごとの行を expand で足す。ADR 0005）
- 統合テスト: `crates/interface/tests/entitlements.rs`（契約の形・月をまたいだ枠のリセット・ワンショットの形・403 / 404）、`crates/server/tests/entitlements.rs`（モックのトークンで通す）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `ContractType`（月額は `subscription`）・`LiveLessonQuota.month`（`YYYY-MM`） | `interface` の `contract.rs` |
| 受講権 ID の UUID のテキスト形式 | `application::entitlement::entitlement_id_uuid` |
| 月額契約 ID の UUID のテキスト形式 | `application::entitlement::subscription_id_uuid` |
| 契約の `SubscriptionStatus` | `interface` の `contract.rs` |

## entitlement: 月額契約の開始と通話の枠の消費（不変条件 1）

いつ・どの予約で枠を消費するか、暦月の区切りは [ADR 0005](../../docs/adr/0005-live-lesson-quota-consumption.md)。

- `POST /subscriptions`（student-app の `startSubscription`）: 生徒だけが呼べる（コーチ・本部は 403）。トークンのアカウントの月額契約（`active`）と、それによる受講権（今の暦月のまだ使っていない枠）を作る（`application::entitlement::StartSubscription`）。受講権がすでにあれば 409 `subscription_already_active`、ゴルフは 422。境界間イベント `SubscriptionStarted` を発行し、行動イベント `subscription.started` を 1 回発火する。決済（billing）は Gen1 の範囲外で、`payment_method_token` は受け取るだけで使わない
- 枠の消費は entitlement が境界間イベントを購読して行う（`LiveLessonQuotaSubscriber`）。`BookingConfirmed` で `ConsumeLiveLessonQuota`、`BookingCancelled` の `consumes_quota: true` でも `ConsumeLiveLessonQuota`（確定で使った予約ならそのまま）、`false` なら `ReleaseLiveLessonQuota`（確定で使った分を戻す）。同じ予約で 2 回は使わない
- 3 回目の拒否: `ConfirmBooking` は確定の前に scheduling のポート `QuotaGate` で entitlement に「もう 1 回予約してよいか」を問い合わせ（実装は `EntitlementQuotaGate`。規則は `Entitlement::may_book_live_lesson`）、否なら 409 `live_lesson_quota_exceeded`（確定しない）。scheduling は回数も上限も知らない。受講権のない生徒（月額契約の前の体験予約など）は制約を受けない
- 購読のつなぎ込み: `server` は境界間イベントを infra の `InMemoryDomainEventBus`（メッセージ基盤のモック。標準出力に 1 行書き、同じプロセスの購読側にその場で渡す。購読側の失敗は標準エラー出力に書いて捨てる）で発行する。現在時刻は `server::router_at(clock)` で差し替えられる（`router()` は OS の時計）
- 統合テスト: `crates/server/tests/live_lesson_quota.rs`（一周: 同じ暦月の 3 回目の確定が 409、翌月は再び確定できる、23 時間前のキャンセルで枠が減り 25 時間前のキャンセルで戻る）、`crates/interface/tests/subscriptions.rs`（契約の形・409・422・403 / 401）、`crates/interface/tests/bookings.rs`（409 `live_lesson_quota_exceeded`）、`crates/infra/tests/in_memory_domain_event_bus.rs`（購読側への配送）、`crates/infra/tests/domain_event_payload.rs`（`SubscriptionStarted` のペイロード）

## live-lesson: 通話セッションの開始・終了

- `POST /live-lesson-sessions`（student-app の `startLiveLessonSession`）: 予約の参加者（生徒・コーチ）だけが呼べる（役割では絞らない。参加者でないアカウントには予約がないのと同じ 404）。予約は scheduling の保存先から読み（ポート `live_lesson::BookingLookup`、実装は `scheduling::SchedulingBookingLookup`）、通話基盤にルームを作らせて参加用の `room_token` を返す。開始すると行動イベント `call.started` を 1 回発火する
- `POST /live-lesson-sessions/{session_id}/end`（`endLiveLessonSession`）: 参加者が終了する。通話基盤がルームを閉じて接続品質の生の指標（`packet_loss_rate`（百分率 0〜100）・`jitter_ms`・`round_trip_ms`・`disconnect_count`）を返し、セッションに記録して行動イベント `call.ended`（4 属性と `duration_sec`）を 1 回発火する。指標と通話品質スコアはレスポンスに載せない。スコアの式はセマンティック層の 1 か所で、バックエンドには置かない（ADR 0002 Q1）
- 不変条件は `domain::live_lesson::LiveLessonSession` が持つ: 確定した予約にしか開始できない（確定前・キャンセル済みは 409 `invalid_booking_state`）、同じ予約のセッションは 1 つ（終了済みを含め 2 回目は 409 `invalid_session_state`）、終了は開始の後（同じ時刻は許す）、終了は 1 回（409 `invalid_session_state`）。並行な開始は保存先が予約ごとに 1 つに絞る（DB では `sessions_booking_id_key`）
- 通話基盤はポート `live_lesson::VideoCallProvider`。`server` は infra の `MockVideoCallProvider` で組み立てる。Blueprint の「回線品質の低いセッションを 15% 混ぜる」に合わせ、15% の確率で品質の低い回線（損失 2〜12%・ジッタ 40〜150 ms・往復 200〜600 ms・切断 0〜3 回）、それ以外は良好な回線（損失 0〜1%・ジッタ 30 ms 以内・往復 150 ms 以内・切断なし）の一様分布から引く。テストは種を固定できる（`MockVideoCallProvider::with_seed`）。録画と実際の Twilio 連携は扱わない
- 保存先はポート `LiveLessonSessionRepository`。`server` は当面インメモリ（`InMemoryLiveLessonSessionRepository`）。Postgres のテーブル `live_lesson.sessions`（expand）は置いたが、実装は予約の保存先を Postgres にするときに足す。予約は scheduling の持ち物なので外部キーは張らない（ADR 0004 の 3）
- 統合テスト: `crates/interface/tests/live_lesson_sessions.rs`（契約の形・確定前の予約の 409・終了で 4 属性が `call.ended` に載ること・参加者以外の 404・422 / 401）、`crates/server/tests/live_lesson_sessions.rs`（モックのトークンと通話基盤のモックで一周）、`crates/infra/tests/mock_video_call_provider.rs`（低品質が 15% 前後。種ごとに ±0.03 の幅で見る）、`crates/infra/tests/schema.rs`（`live_lesson.sessions` の制約。`--include-ignored`）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `LiveLessonSessionStatus` | `interface` の `contract.rs` |
| 通話セッション ID の UUID のテキスト形式 | `application::live_lesson::session_id_uuid` |
| モックの `room_token`（`mock-room.<セッション ID>.<参加者 ID>`） | `infra` の `MockVideoCallProvider` |

## async-lesson: 動画の送付と添削の返却

- `POST /videos`（student-app の `submitVideo`）: 生徒だけが呼べる（コーチ・本部は 403）。送る生徒はトークンのアカウント。動画の保存先（ポート `async_lesson::VideoStorage`）に本体の PUT 先を発行させ、`upload_url` で返す。動画本体はクライアントが保存先に直接 PUT し、バックエンドは通さない。送付すると行動イベント `video.submitted` を 1 回発火する。送付の回数は数えない（月額は送り放題）。ゴルフ（golf-app の範囲）・`content_type` が `video/mp4` / `video/quicktime` でない・`coach_id` が UUID でない・`note` が 1000 文字を超えるときは 422。Gen1 はアップロードの完了を受け取らないので、レスポンスの `status` は常に `awaiting_upload`。`note` は長さだけを見て保存しない
- `GET /videos/{video_id}/review`（`getVideoReview`）: 動画の当事者（送った生徒・送り先のコーチ）だけが読める（役割では絞らない。当事者でないアカウント・ない動画・UUID でない ID には 404）。コーチが返すまでは `status: in_progress`（未返却）で、`review_id`・`comment`・`returned_at` は null
- `POST /videos/{video_id}/review`（admin-web の `returnVideoReview`）: コーチだけが呼べ（生徒・本部は 403）、返せるのは自分に送られた動画だけ（それ以外は 404）。返却は 1 回（2 回目は 409 `invalid_video_state`）、コメントが空か 4000 文字を超えると 422。返却すると境界間イベント `VideoReviewReturned` を発行し（notification が生徒に通知する）、行動イベント `video_review.returned` を 1 回発火する。返却のリードタイムをセマンティック層で `occurred_at - submitted_at` と計算できるよう、送付の時刻 `submitted_at` も載せる
- 不変条件は `domain::async_lesson::Video` が持つ: 返せるのは送り先のコーチだけ、返却は 1 回、返却は送付の後（同じ時刻は許す）、着手できない動画は返せない。状態（`Submitted` / `Reviewed`）は添削の有無で決まる
- 不変条件 2（ゴルフの添削はワンショット決済が確定した動画にしか着手できない）: 規則は `domain::entitlement::ContractType::may_start_review` の 1 か所（月額は常に可、ワンショットは決済が確定した動画だけ）。返却の前にポート `async_lesson::ReviewGate` で問い合わせ、実装の `entitlement::EntitlementReviewGate` は動画のスポーツの契約種別（`ContractType::for_sport`）で答える。Gen1 はワンショットの決済（billing）を実装しないので、ゴルフの動画には常に 409 `review_payment_not_confirmed`。Gen1 は着手と返却を分けないので、返却が着手を兼ねる
- 保存先はポート `VideoRepository`。`server` は当面インメモリ（`InMemoryVideoRepository`）で、動画の保存先は infra の `MockVideoStorage`（`https://mock-storage.invalid/videos/<動画 ID>` を返す。署名・有効期限はなく、PUT しても何も保存されない）。Postgres のテーブル `async_lesson.videos` / `video_reviews`（expand）は置いたが、実装はアカウントの保存先を Postgres にするときに足す
- 送り先のコーチがいるか・生徒のスポーツと合うかは確かめない（アカウントを読むポートがまだない。Postgres の実装では `videos.coach_id` の外部キーが `identity.coaches` を指す）。契約の `submitVideo` の 404 はそのときに返す
- 統合テスト: `crates/interface/tests/videos.rs`（契約の形・送付 → 返却の一連と返却前の未返却・`video.submitted` / `video_review.returned` の属性・ゴルフの 409・403 / 404 / 409 / 422 / 401）、`crates/server/tests/videos.rs`（モックのトークンと保存先のモックで一周）、`crates/infra/tests/mock_video_storage.rs`、`crates/infra/tests/schema.rs`（`async_lesson` の制約。`--include-ignored`）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `content_type`・`VideoReviewStatus`・送付のレスポンスの `VideoStatus`（`awaiting_upload`） | `interface` の `contract.rs` |
| 動画 ID・添削 ID の UUID のテキスト形式 | `application::async_lesson::video_id_uuid`・`video_review_id_uuid` |
| モックのアップロード URL | `infra` の `MockVideoStorage` |

## レッスンの完了記録（is_first_lesson と LessonCompleted）

置き場所の判断（集約の外に lesson のコンテキストを作らない・テーブルは元の集約のスキーマ）と、glossary の `lesson.context` を見直す Question は [ADR 0006](../../docs/adr/0006-lesson-completion-placement.md)。

- `POST /lessons/completions`（student-app の `recordLessonCompletion`）: 同期レッスンは `{"lesson_type": "live_lesson", "session_id": ...}`、非同期レッスンは `{"lesson_type": "async_lesson", "video_id": ...}`。記録できるのは元の通話セッション・動画の当事者（生徒・コーチ）だけで、役割では絞らない（それ以外のアカウント・ない ID は 404）。ID が UUID でない・判別子に合わない ID・知らない項目は 422。終了していないセッションは 409 `invalid_session_state`、添削が返されていない動画は 409 `invalid_video_state`、同じセッション・動画の 2 回目は 409 `lesson_already_completed`
- 完了記録は元の集約が発行する: `domain::live_lesson::LiveLessonSession::complete` と `domain::async_lesson::Video::complete` が `domain::lesson::LessonCompletion` を返す。`is_first_lesson` はその生徒の完了記録が同期・非同期あわせて 0 件のときだけ true で、判定は `LessonCompletion` の 1 か所
- `application::lesson::RecordLessonCompletion` は元の集約を読み、保存先（ポート `LessonCompletionRepository::record`。同じ生徒の既存の記録を渡し、読み取りから保存までを生徒ごとに直列にする）に保存してから、行動イベント `lesson.completed`（`lesson_type`・`is_first_lesson`）を 1 回発火し、境界間イベント `LessonCompleted`（billing・review・analytics 向け）を 1 回発行する。billing・review の購読はまだない
- 保存先は当面インメモリ（`InMemoryLessonCompletionRepository`）で、通話セッション・動画の保存先と同じ中身を読む。Postgres のテーブル `live_lesson.lesson_completions` / `async_lesson.lesson_completions`（expand）は置いたが、実装は通話セッション・動画の保存先を Postgres にするときに足す（2 つのテーブルをまたいだ初回の重なりは生徒ごとの advisory lock で防ぐ）
- 統合テスト: `crates/interface/tests/lessons.rs`（契約の形・通話と動画の両方からの完了・同じ生徒の 2 回目で `is_first_lesson` が false・イベント・409 / 404 / 422 / 401）、`crates/server/tests/lessons.rs`（予約 → 通話 → 完了、動画 → 添削 → 完了の一周）、`crates/infra/tests/domain_event_payload.rs`（`LessonCompleted` のペイロード）、`crates/infra/tests/schema.rs`（`lesson_completions` の制約。`--include-ignored`）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `LessonType`（`live_lesson` / `async_lesson`） | `interface` の `contract.rs` |
| 完了記録 ID の UUID のテキスト形式 | `application::lesson::lesson_id_uuid` |

## notification: 予約の確定・添削の返却・メッセージの受信でのプッシュ通知（モック）

- 通知のきっかけは境界間イベントの購読だけで、他のコンテキストは notification を直接呼ばない（`specs/context-map.yaml` の向き）。`application::notification::PushNotificationSubscriber` が次の 3 つを購読し、`domain::notification::PushNotification` を送る（受け取る人は `recipient()`）。ほかのイベントでは送らない（キャンセルの通知はまだない）
  - `BookingConfirmed`（scheduling）→ 生徒に `PushNotification::BookingConfirmed`（予約の確定）、予約を受けたコーチに `PushNotification::BookingReceived`（新しい予約）の 2 件（どちらもコーチ・予約の ID）
  - `VideoReviewReturned`（async-lesson。添削の返却 `ReturnVideoReview` が発行）→ 動画を送った生徒に `PushNotification::VideoReviewReturned`（コーチ・動画・添削の ID）
  - `MessageSent`（messaging。メッセージの送信 `SendMessage` が発行）→ 受け手（送り手でない側の当事者。`sender_role` が student ならコーチ、coach なら生徒）に `PushNotification::MessageReceived`（スレッド・メッセージの ID と送り手。本文は載せない）
- 送り先の基盤はポート `PushSender`。Gen1 の実装は infra の `MockPushSender`（実際の端末には届けず、送った通知を `sent()` で読めるように記録し、`push_notification <受け取るアカウントの ID> <種類> <対象の ID>` の 1 行で標準出力に書く。種類と対象は `booking_confirmed <予約の ID>`・`booking_received <予約の ID>`・`video_review_returned <動画の ID>`・`message_received <スレッドの ID>`）。テスト用に `in_memory::RecordingPushSender`。表示する文言は扱わない（本物の基盤につなぐときに infra のアダプタで決める）
- 送る前に受け取る人の通知の受け取り設定（下の「通知の受け取り設定」）を読み、その通知の種類（`PushNotification::kind()`）がオフなら送らない
- 通知そのものは保存しない。同じイベントの再送で 2 回送らないよう、送った通知を `event_id` と組にしてプロセスのメモリに覚える（受け取り設定がオフで送らなかった通知も覚え、あとでオンに戻しても再送では送らない。受け取り設定を読めなかった通知は送れなかった通知と同じく再送で送り直す。1 イベントで 2 件送る予約の確定も通知ごとに覚え、送れなかった通知だけを再送で送り直す。1 件が送れなくても残りは送り、障害を返す）。プロセスを再起動すると忘れる
- 組み立ては `server` の `InMemoryDomainEventBus` の購読側（`MessageThreadSubscriber` と組にして足した）。`ReturnVideoReview`・`SendMessage` にも同じバスを発行先として渡す。テストから送り先を差し替える組み立ては `server::router_with_push_sender`
- テスト: `crates/domain/src/notification/push_notification.rs`（受け取る人）、`crates/application/src/notification/push_notification_subscriber.rs`（予約の確定は生徒とコーチに 1 件ずつ・1 イベント 1 回・メッセージは送り手でない側へ・ほかのイベントで送らない・失敗後の再送・片方だけ失敗したときは失敗した方だけを再送で送る）、`crates/application/src/async_lesson/return_video_review.rs`・`messaging/send_message.rs`（保存したら発行し、拒否では発行しない）、`crates/infra/tests/domain_event_payload.rs`（`VideoReviewReturned` / `MessageSent` のペイロードが AsyncAPI の `required` と一致すること）、`crates/infra/tests/mock_push_sender.rs`（記録と 1 行の形）、`crates/server/tests/notifications.rs`（予約の作成では送らず、確定で生徒とコーチに 1 件ずつ、キャンセルでは送らない。動画の送付では送らず、返却で生徒に 1 件。コーチの送信は生徒へ、生徒の送信はコーチへ、拒否された送信では送らない）

### 通知の受け取り設定（D16）

- アカウントごと・通知の種類ごとに受け取るか（オン / オフ）を決める（glossary の `notification_settings`、domain の `NotificationSettings`）。既定はすべてオンで、変えたことのない種類は保存しない。種類（`NotificationKind`）は通知のきっかけごとに 3 つ: `BookingConfirmed`（予約の確定。生徒への `BookingConfirmed` とコーチへの `BookingReceived` の両方）・`VideoReviewReturned`（添削の返却）・`MessageReceived`（メッセージの受信）
- student-app の `GET /notification-settings/me`（`getMyNotificationSettings`）と `PUT /notification-settings/me`（`updateMyNotificationSettings`）。生徒だけ（ほかの役割は 403）。PUT は書いた種類だけを変え、書かなかった種類は今のまま（真偽値でなければ 422）。行動イベントは発火しない。契約の項目名（`booking_confirmed` / `video_review_returned` / `message_received`）との対応は `interface` の `notification_settings.rs` だけ
- 保存先はポート `NotificationSettingsRepository`（保存していなければ既定を返す。変更は読み取りから保存までを直列にする）。Postgres は `notification.notification_settings`（アカウント × 種類で 1 行。マイグレーション `20261028000001_expand_notification_settings.sql`、種類の文字列は `infra::notification_values`）。`PgNotificationSettingsRepository` はアカウントごとの advisory lock の中で読み、すべての種類の行を upsert する。`router_at` などの組み立ては当面インメモリ（`InMemoryNotificationSettingsRepository`）、`router_with_postgres` は Postgres
- コーチの受け取り設定を変える操作（admin-web）はまだない。コーチへの通知（`BookingReceived`・コーチへの `MessageReceived`）も同じ表を読むので、行がなければすべて送る
- テスト: `crates/domain/src/notification/notification_settings.rs`（既定はすべてオン・オフとオンの切り替え）、`crates/application/src/notification/`（`get_notification_settings.rs`・`update_notification_settings.rs`、`push_notification_subscriber.rs` のオフなら送らず再送でも送らない・オフの種類だけを止める・設定を読めなければ再送で送る）、`crates/infra/tests/pg_notification_settings_repository.rs`（既定・往復・並行な変更が互いを消さない）・`schema.rs`（種類の CHECK・1 アカウント 1 種類 1 行・アカウントへの外部キー）、`crates/interface/tests/notification_settings.rs`（契約の形・既定・部分の変更・422・403・401）、`crates/server/tests/notifications.rs`（オフにした生徒には送らずコーチには送る・Postgres の組み立てで保存される）

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
4. 同じ暦月に 3 回目の通話予約の確定が 409 `live_lesson_quota_exceeded`（月額契約の前に確定した体験予約は枠を使わないので、契約後の 2 回を確定してからの 3 回目）

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
