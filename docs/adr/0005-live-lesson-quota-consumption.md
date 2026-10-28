# ADR 0005: ビデオ通話の暦月の枠をいつ・どの予約で消費するか、暦月をどこで区切るか

- 状態: 提案（2026-10-26、T11 / Issue #34。暦月のタイムゾーンは建設者の確認待ち）
- 関係: `specs/product-context.md`（不変条件 1・3）、ADR 0002 Q2（キャンセル規定の判定は scheduling、消費は entitlement）、ADR 0004 の 4（「T11 で決める」とした 2 点）、`specs/contracts/events/asyncapi.yaml`（BookingConfirmed / BookingCancelled）

## 背景

不変条件 1「月額契約の生徒のビデオ通話は暦月 2 回まで。3 回目は予約できない」と、キャンセル規定「開始 24 時間前以降のキャンセルは回数を消費する」（判定は scheduling が `BookingCancelled.consumes_quota` に載せる）を両立させる。ADR 0004 の 4 で「枠の消費をいつ・どの予約で行うか、暦月をどのタイムゾーンで区切るか」を T11 に送っていた。

素直に「確定で 1 回、`consumes_quota: true` のキャンセルでもう 1 回」消費すると、確定した予約を 23 時間前にキャンセルすると 2 回分になり、25 時間前にキャンセルしても確定の 1 回分が残る（キャンセル規定が確定済みの予約に効かない）。

## 決定

1. **枠は「その月に枠を使った予約の集合」で持つ**。使った回数はその数で、上限 2 は `domain::entitlement::LiveLessonQuota::LIMIT` の 1 か所だけ（D37 / Issue #174 で、上限は契約プランの `domain::entitlement::SubscriptionPlan::live_lessons_per_month` の 1 か所に移した。standard 2 回・plus 4 回。`LiveLessonQuota` はプランを持ち、上限をそこから受け取る。以下の「3 回目」は standard の場合で、plus では 5 回目）。同じ予約で 2 回は使わない（同じイベントが 2 回届いても数が変わらない）
2. **消費・戻しの契機**（entitlement が境界間イベントを購読して行う。`application::entitlement::LiveLessonQuotaSubscriber`）
   | 出来事 | 枠 |
   |---|---|
   | `BookingConfirmed` | その予約で 1 回使う |
   | `BookingCancelled`（`consumes_quota: true`。開始 24 時間前以降） | その予約で使ったことにする（確定で使っていればそのまま、確定前の予約なら 1 回使う） |
   | `BookingCancelled`（`consumes_quota: false`） | その予約で使った分を戻す（確定前の予約なら何もしない） |
   - 結果として「確定してキャンセルしなかった予約」と「24 時間前以降にキャンセルした予約」が 1 回ずつ数えられる。回数を消費するかの判定は scheduling の `CancellationPolicy` だけで、entitlement は `consumes_quota` に従う（ADR 0002 Q2・不変条件 3）
3. **3 回目の拒否は予約の確定で行う**。scheduling の `ConfirmBooking` は確定の前にポート `scheduling::QuotaGate` で「もう 1 回予約してよいか」を問い合わせ、否なら 409 `live_lesson_quota_exceeded`。判定の規則（`Entitlement::may_book_live_lesson`）は entitlement にあり、scheduling は回数も上限も知らない。予約の作成（`pending`）では問い合わせない（受講権を消費しないので）。契約の `createBooking` の説明にあった「枠を超える予約は 409」は `confirmBooking` に移した
4. **どの暦月の枠か**は、出来事の時刻（`BookingConfirmed.occurred_at`・`BookingCancelled.cancelled_at`）で決める。予約したレッスンの開始時刻ではない（「3 回目は予約できない」を予約の時点で数える。T10 の受講権が「最後に記録した暦月の枠」を 1 つだけ持つ形とも合う）
5. **暦月は日本時間（UTC+9。夏時間なし）で区切る**。利用者は日本の生徒で、UTC で区切ると日本時間の 1 日 9 時まで前の月になる。区切りは `application::entitlement::calendar_month` の 1 か所（T10 の UTC から変えた）
6. 受講権のない生徒（月額契約の前の体験予約など）とワンショットの受講権は、暦月の回数の制約を受けない（不変条件 1 は月額の生徒の規則）。体験予約も月額契約の後なら他の予約と同じに数える

## 結果

- `QuotaGate` の問い合わせと購読による消費は別の手順なので、同じ生徒の確定が並行すると両方が問い合わせを通りうる。後の消費は entitlement が拒否し、購読側の失敗として記録する（予約は確定済みのまま）。Gen1 の生徒アプリの操作では起きにくいので許し、メッセージ基盤を選ぶときに見直す
- 月をまたいだ戻し（10 月に確定し、11 月に 25 時間前より前にキャンセル）は 11 月の枠に何もしない（10 月の記録は持っていない）
- Postgres の `entitlement.live_lesson_quotas`（ADR 0004）は残回数の列だけで、予約の集合を持てない。Postgres の実装を足すときは、予約ごとの消費の行（例: `(entitlement_id, month, booking_id)`）を expand で足す

## 選ばなかった案

- **確定と遅いキャンセルの両方で数える（予約ごとの重複を見ない）**: 確定済みの予約の遅いキャンセルが 2 回分になり、早いキャンセルでも 1 回分が残る
- **レッスンの開始時刻の暦月で数える**: 「暦月のビデオ通話が 2 回まで」に忠実だが、受講権が複数の月の枠を持つ必要があり、`BookingCancelled` に開始時刻がない（契約の変更が要る）。予約の時点で数える案より変更が大きい
- **確定の中で消費まで同期で行う（scheduling から entitlement のユースケースを直接呼ぶ）**: 並行の窓はなくなるが、scheduling が entitlement の消費の手順に依存する。ADR 0002 Q2 の「消費の実行は entitlement が購読して行う」と食い違う
