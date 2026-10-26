# apps/backend

Rally のバックエンド（Rust / axum / PostgreSQL）の Cargo ワークスペース。層の規約と依存方向は [ADR 0003](../../docs/adr/0003-backend-layers.md)。

## 構成

| パス | 層 | 依存してよいクレート |
|---|---|---|
| `crates/domain/` | ドメイン。純粋（I/O なし）。`identity`（アカウント・役割・スポーツと不変条件）・`scheduling`（スロットと、重なりを判定する集約 `CoachCalendar`・予約 `Booking`・キャンセル規定 `CancellationPolicy`）・`entitlement`（契約種別 `ContractType`・受講権 `Entitlement`・通話の暦月の枠 `LiveLessonQuota`・暦月 `YearMonth`・月額契約 `Subscription`）・`live_lesson`（通話セッション `LiveLessonSession`・接続品質の生の指標 `ConnectionQuality`） | なし |
| `crates/application/` | ユースケースとポート（`identity::RegisterAccount`・`AccountRepository`・`TokenIssuer`・`TokenVerifier`・`scheduling::PublishSlot`・`SearchSlots`・`SlotRepository`・`CreateBooking`・`ConfirmBooking`・`CancelBooking`・`BookingRepository`・`QuotaGate`・`entitlement::GetEntitlement`・`StartSubscription`・`ConsumeLiveLessonQuota`・`ReleaseLiveLessonQuota`・`LiveLessonQuotaSubscriber`・`EntitlementQuotaGate`・`EntitlementRepository`・`live_lesson::StartLiveLessonSession`・`EndLiveLessonSession`・`LiveLessonSessionRepository`・`VideoCallProvider`・`BookingLookup`（実装は `scheduling::SchedulingBookingLookup`）・`AnalyticsSink`・`DomainEventPublisher`・`DomainEventSubscriber`・`Clock`）。ポートのインメモリ実装（`in_memory`）。ID と UUID の変換（`identity::account_id_uuid`・`scheduling::slot_id_uuid`・`scheduling::booking_id_uuid`・`entitlement::entitlement_id_uuid`・`entitlement::subscription_id_uuid`・`live_lesson::session_id_uuid`）・時刻から暦月（`entitlement::calendar_month`）・障害の型（`Unavailable`） | domain |
| `crates/infra/` | Postgres（`migrations/`・`PgSlotRepository`）・モックの外部連携（`MockTokenService`: アクセストークン、`LoggingDomainEventPublisher`: 境界間イベントを標準出力に書く、`InMemoryDomainEventBus`: 境界間イベントを同じプロセスの購読側に渡す、`MockVideoCallProvider`: 通話基盤。終了時に接続品質の指標を乱数で生成する）・DB とトークンと境界間イベントのペイロードの文字列の対応表（`identity_values`・`scheduling_values`・`domain_event_payload`） | domain・application |
| `crates/interface/` | axum の HTTP（`GET /healthz`、`POST /accounts`、`GET /slots`、`POST /slots`、`POST /bookings`、`POST /bookings/{booking_id}/confirm`、`POST /bookings/{booking_id}/cancel`、`POST /subscriptions`、`GET /entitlements/me`、`POST /live-lesson-sessions`、`POST /live-lesson-sessions/{session_id}/end`）・認証（`AuthenticatedAccount`・`require_role`）・契約の文字列の対応表と API の文言 | domain・application（infra は不可） |
| `crates/server/` | 組み立て用（composition root）。interface に infra の実装を渡し、HTTP サーバーを起動する | domain・application・infra・interface |
| `crates/architecture/` | 依存方向の検査（テストだけ） | なし |

ツールチェーンは `rust-toolchain.toml`（stable、rustfmt・clippy 付き）。

## データベース

ローカルの Postgres は `compose.yaml`（Docker Compose）で起動する。スキーマは `crates/infra/migrations/` の sqlx のマイグレーションで、変更は expand → 移行 → contract の 3 段に分ける（[ADR 0004](../../docs/adr/0004-schema-expand-contract.md)）。

```sh
docker compose up -d --wait                       # 127.0.0.1:5432 に Postgres 16
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

ポートは組み立て用のクレート `server` で組み立てる（ADR 0003 の決定 5）。アカウント・スロット・予約・受講権・通話セッションの保存先と行動トラッキングは当面 `application::in_memory` のインメモリ実装なので、登録したアカウントや公開した枠はプロセスを止めると消える。Postgres の実装は infra に置き、`server` で差し替える。

## identity の不変条件

`domain::identity` がモデルの生成時に検証し、破れば `DomainError` を返す。

- 役割（生徒・コーチ・本部）とスポーツの組み合わせ: 生徒はちょうど 1 つ、コーチは 1 つ以上（提供スポーツの集合）、本部はなし
- スポーツは Blueprint の 5 種類（tennis / golf / baseball / soccer / basketball）のどれか。student-app の `POST /accounts` はさらにゴルフを 422 で断る（契約の `Sport` がゴルフを含まない）
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

## CI で走らせるコマンド

`apps/backend/` で次の 3 つを順に走らせ、どれかが落ちたらマージしない（L1）。依存方向の検査は `cargo test` に含まれる（`crates/architecture/tests/dependency_direction.rs`）。

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
