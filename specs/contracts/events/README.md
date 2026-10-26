# specs/contracts/events/

境界づけられたコンテキスト間のドメインイベントを AsyncAPI で定義する置き場所。定義本体はまだ書かない。

## 例（名前のみ。発行元 → 購読先は context-map.yaml の id）

- `LessonCompleted`: live-lesson / async-lesson → billing / review / analytics
- `BookingConfirmed`: scheduling → notification / analytics
- `VideoReviewReturned`: async-lesson → notification / review
