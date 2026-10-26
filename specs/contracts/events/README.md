# contracts/events/

境界づけられたコンテキスト間のドメインイベントを AsyncAPI で定義する場所。ここでは例を名前で挙げるだけで、定義本体はまだ書かない。

- `LessonCompleted`: レッスン完了 → billing / review / analytics
- `BookingCancelled`: 予約キャンセル → entitlement / notification / analytics
- `ReviewSubmitted`: 添削の返却 → notification / analytics
