# apps/backend

Rally のバックエンド（Rust / axum / PostgreSQL）の Cargo ワークスペース。層の規約と依存方向は [ADR 0003](../../docs/adr/0003-backend-layers.md)。

## 構成

| パス | 層 | 依存してよいクレート |
|---|---|---|
| `crates/domain/` | ドメイン。純粋（I/O なし）。`identity`（アカウント・役割・スポーツと不変条件）・`scheduling`（スロットと、重なりを判定する集約 `CoachCalendar`・予約 `Booking`・キャンセル規定 `CancellationPolicy`） | なし |
| `crates/application/` | ユースケースとポート（`identity::RegisterAccount`・`AccountRepository`・`TokenIssuer`・`TokenVerifier`・`scheduling::PublishSlot`・`SearchSlots`・`SlotRepository`・`CreateBooking`・`ConfirmBooking`・`CancelBooking`・`BookingRepository`・`AnalyticsSink`・`DomainEventPublisher`・`Clock`）。ポートのインメモリ実装（`in_memory`）。ID と UUID の変換（`identity::account_id_uuid`・`scheduling::slot_id_uuid`・`scheduling::booking_id_uuid`）・障害の型（`Unavailable`） | domain |
| `crates/infra/` | Postgres（`migrations/`・`PgSlotRepository`）・モックの外部連携（`MockTokenService`: アクセストークン、`LoggingDomainEventPublisher`: 境界間イベントを標準出力に書く）・DB とトークンと境界間イベントのペイロードの文字列の対応表（`identity_values`・`scheduling_values`・`domain_event_payload`） | domain・application |
| `crates/interface/` | axum の HTTP（`GET /healthz`、`POST /accounts`、`GET /slots`、`POST /slots`、`POST /bookings`、`POST /bookings/{booking_id}/confirm`、`POST /bookings/{booking_id}/cancel`）・認証（`AuthenticatedAccount`・`require_role`）・契約の文字列の対応表と API の文言 | domain・application（infra は不可） |
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

ポートは組み立て用のクレート `server` で組み立てる（ADR 0003 の決定 5）。アカウント・スロット・予約の保存先と行動トラッキングは当面 `application::in_memory` のインメモリ実装なので、登録したアカウントや公開した枠はプロセスを止めると消える。Postgres の実装は infra に置き、`server` で差し替える。

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
- `POST /bookings/{booking_id}/confirm`（`confirmBooking`）: 生徒だけが呼べ、確定できるのは自分の予約だけ（別の生徒の予約・UUID でない ID は 404）。確定するとスロットが `booked` になり（検索に出なくなる）、境界間イベント `BookingConfirmed` を発行し、行動イベント `booking.confirmed` を 1 回発火する。`pending` でなければ 409 `invalid_booking_state`
- 不変条件は `domain::scheduling::Booking` が持つ: 公開中（`open`）のスロットにしか予約できない（予約済みは 409 `slot_already_booked`、閉じた枠はない枠と同じ 404）、1 スロットに有効な（キャンセルされていない）予約は 1 件（二重予約は 409 `slot_already_booked`）、体験予約はその生徒の初回だけ（キャンセルされていない予約が 1 件でもあれば 409 `trial_already_used`）
- ポート `BookingRepository` は予約とスロットを一緒に読み書きし、判定は domain に任せて読み取りから保存までを直列にする。`server` は当面インメモリ（`InMemoryBookingRepository`。スロットの保存先と中身を共有）。Postgres の実装はアカウント・スロットの保存先を Postgres にするときに足す（DB は `bookings_one_active_per_slot` でも二重予約を防ぐ）
- 境界間イベントはポート `DomainEventPublisher`（発行の失敗でユースケースを失敗させない）。Gen1 はメッセージ基盤を選定していないので、`server` は infra の `LoggingDomainEventPublisher`（`domain_event BookingConfirmed {...}` の 1 行を標準出力に書くだけ）で組み立てる
- 統合テスト: `crates/interface/tests/bookings.rs`（契約の形・二重予約・体験予約の 1 回限り・確定でスロットが埋まること・403 / 404 / 409）、`crates/server/tests/bookings.rs`（モックのトークンで一周の入口を通す）、`crates/infra/tests/domain_event_payload.rs`（`BookingConfirmed` のペイロードが AsyncAPI の `required` と一致すること）

| 何の文字列か | 置き場所 |
|---|---|
| 契約の `BookingStatus` | `interface` の `contract.rs` |
| 予約 ID の UUID のテキスト形式 | `application::scheduling::booking_id_uuid` |
| 境界間イベントのペイロード（AsyncAPI） | `infra::domain_event_payload` |

## scheduling: 予約のキャンセルとキャンセル規定

- `POST /bookings/{booking_id}/cancel`（`cancelBooking`）: 生徒だけが呼べ、キャンセルできるのは自分の予約（`pending` / `confirmed`）だけ（別の生徒の予約・UUID でない ID は 404、キャンセル済みは 409 `invalid_booking_state`）。確定済みならスロットを `open` に戻す（検索に戻り、別の生徒が予約できる）。本文の `reason` は任意で、Gen1 では保存しない
- キャンセル規定（開始の何時間前以降なら回数を消費するか）は `domain::scheduling::CancellationPolicy` の 1 か所だけに置く（不変条件 3。ADR 0002 Q2）。判定の境界（ちょうどの時刻は消費する側）と前後のテストもそこにある。閾値を application・interface・infra や entitlement に写さない
- 判定の結果は境界間イベント `BookingCancelled` の `consumes_quota` に載せて発行する（scheduling → entitlement / notification / analytics）。受講権（ビデオ通話の暦月の枠）の消費の実行は entitlement が購読して行う（T11）。API のレスポンスと行動イベントには判定を載せない
- 行動イベント `booking.cancelled` を 1 回発火する。`hours_before_start` はキャンセルした時刻から開始時刻までの時間を時間単位で切り捨てた整数（開始後のキャンセルは負）
- 統合テスト: `crates/interface/tests/bookings.rs`（契約の形・23 / 25 時間前の `consumes_quota`・スロットが空くこと・403 / 404 / 409）、`crates/server/tests/bookings.rs`（モックのトークンで通す）、`crates/infra/tests/domain_event_payload.rs`（`BookingCancelled` のペイロードが AsyncAPI の `required` と一致すること）

## CI で走らせるコマンド

`apps/backend/` で次の 3 つを順に走らせ、どれかが落ちたらマージしない（L1）。依存方向の検査は `cargo test` に含まれる（`crates/architecture/tests/dependency_direction.rs`）。

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
