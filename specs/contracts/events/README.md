# specs/contracts/events/

境界づけられたコンテキスト間のドメインイベントを AsyncAPI 3.0 で定義する置き場所。本体は `asyncapi.yaml`。

## 定義済みのイベント（発行元 → 購読先は context-map.yaml の id）

- `BookingConfirmed`: scheduling → notification / analytics
- `BookingCancelled`: scheduling → entitlement / notification / analytics（キャンセル規定の判定 `consumes_quota` を載せる。docs/adr/0002 Q2）
- `LessonCompleted`: live-lesson / async-lesson → billing / review / analytics
- `SubscriptionStarted`: entitlement → billing / analytics

## 規約

- 1 イベント 1 チャネル。発行元・購読先はチャネルの `x-publisher` / `x-subscribers` に context id の配列で書く
- ペイロードは必須属性だけ（`event_id`・`occurred_at`・関係する `student_id` / `coach_id`・対象の id と、購読側が判断に要る事実（`BookingCancelled` の `consumes_quota` など））。属性名は `specs/analytics/events.yaml` と揃える
- メッセージ基盤は選定しない（`address` はチャネル名と同じにしておく）

## 未定義の候補

- `VideoReviewReturned`: async-lesson → notification / review
