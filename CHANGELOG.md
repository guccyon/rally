# 変更履歴

Keep a Changelog の形。利用者（生徒・コーチ・本部）や API の利用者に見える変更を `[Unreleased]` に足し、リリース時に版の項へ移す。工場のワーカーも、利用者に見える変更を含む PR ではここを更新する（rubric DOC-002）。

## [Unreleased]

### 追加
- バックエンドの骨格: Cargo ワークスペースと層（domain / application / infra / interface / server）、依存方向テスト、`GET /healthz`（#19）
- 契約: student-app の OpenAPI（11 操作、`x-context` 付き）と境界間イベントの AsyncAPI（BookingConfirmed / LessonCompleted / SubscriptionStarted）（#20, #21）
- DB スキーマの初版（identity / scheduling / entitlement）と expand/contract のマイグレーション基盤（#22）
- `POST /accounts`: 生徒・コーチ・本部のアカウント登録。登録時に `account.registered` の行動イベントを発火（#28）
- 認証: ロールを載せたアクセストークンの発行・検証（Gen1 はモック。形式 `mock.<ロール>.<アカウント ID>`）とロールによる認可。サーバーの起動は `cargo run -p server`（#29）
- スロット（コーチの予約枠）: `POST /slots`（コーチ。同じコーチの枠の重なりは 409）と `GET /slots`（スポーツ・コーチ・期間で絞り、カーソルでページング）。公開時に `slot.published` を発火（#30）
- 予約: `POST /bookings`（体験予約を含む。二重予約は 409 `slot_already_booked`、2 回目の体験予約は 409 `trial_already_used`）と `POST /bookings/{booking_id}/confirm`（確定でスロットが予約済みになり、境界間イベント `BookingConfirmed` を発行）。`booking.created` / `booking.confirmed` を発火（#31）
- 予約のキャンセル: `POST /bookings/{booking_id}/cancel`（確定済みならスロットが公開中に戻る。キャンセル済みは 409 `invalid_booking_state`）。キャンセル規定（開始 24 時間前以降は回数を消費）の判定を載せた境界間イベント `BookingCancelled`（`consumes_quota`）を発行し、`booking.cancelled` を発火（#32）
- 受講権の照会: `GET /entitlements/me`（生徒だけ。契約種別と、今の暦月のビデオ通話の枠 `live_lesson_quota`。月をまたげば `used` は 0 から。受講権がなければ 404）。契約に `getMyEntitlement` を追加（#33）
- 月額契約の開始: `POST /subscriptions`（生徒だけ。受講権がすでにあれば 409 `subscription_already_active`、ゴルフは 422。決済は Gen1 では扱わない）。境界間イベント `SubscriptionStarted` を発行し、`subscription.started` を発火。ビデオ通話の暦月 2 回の枠を予約の確定で使い（3 回目の確定は 409 `live_lesson_quota_exceeded`）、開始 24 時間前以降のキャンセルは枠を使ったことに、それより前のキャンセルは確定で使った枠を戻す。暦月は日本時間で区切る（#33 の UTC から変更）（#34）
- 通話セッション: `POST /live-lesson-sessions`（確定した予約の参加者（生徒・コーチ）だけ。確定前の予約は 409 `invalid_booking_state`、同じ予約の 2 回目は 409 `invalid_session_state`、`booking_id` が UUID でなければ 422）と `POST /live-lesson-sessions/{session_id}/end`（終了済みは 409 `invalid_session_state`）。通話基盤はモックで、終了時に接続品質の生の指標（約 15% は低品質の回線）を生成する。`call.started` / `call.ended`（4 属性）を発火（#37）

### 変更
- `access_token` の形式が `mock-<id>` から `mock.<ロール>.<id>` に変わった。旧形式のトークンは検証に通らない（#29）
