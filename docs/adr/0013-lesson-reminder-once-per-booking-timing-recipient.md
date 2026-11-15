# ADR 0013: レッスンのリマインドは定期の処理が作り、(予約, 種類, 宛先) の一意で 1 回だけにする

- 状態: 提案（工場）（2026-11-14、Issue #1463 の t2 / #1476）
- 関係: ADR 0003（層。定期の処理の組み立ては server、ユースケースは application、窓の判定は domain）、ADR 0004（expand の緩和で足す・付け直す）、ADR 0009（notification は汎用の区分。予約の事実は scheduling から受ける）、`specs/contracts/api/student-app/openapi.yaml`（`listMyNotifications`・`updateMyNotificationSettings`・`NotificationSettings.lesson_reminder`・`Notification.booking_id`）、`specs/contracts/api/admin-web/openapi.yaml`（コーチの `listMyNotifications`）、`specs/contracts/events/asyncapi.yaml`（`BookingConfirmed`・`BookingCancelled` の notification の説明）

## 文脈

確定した同期レッスン（ビデオ通話）の予約について、開始の前日と開始 1 時間前に、生徒と担当コーチへリマインドのお知らせを届ける（#1463。契約は #1475 で済み）。リマインドは予約のイベントでは起きず、時刻の経過で起きる。これまでの notification のお知らせは、どれも境界間イベントの購読（`PushNotificationSubscriber`）か本部の操作がきっかけだった。

時刻で動く処理は、決まった間隔で何度も回る。同じ窓の中で何度回っても、同じ予約の同じリマインドを同じ人に 2 件作ってはいけない。サーバーが止まっていた・複数の台で動くといった場合にも重複しない仕組みが要る。

あわせて、キャンセル・コーチのお休みで消えた予約には作らない。生徒の受け取り設定で `lesson_reminder` が false ならお知らせも作らない（ほかの種類は false でもお知らせに残る。その例外）。コーチには受け取り設定がないので常に作る。

## 決定

1. **リマインドは定期の処理が作る**。application の `notification::SendLessonReminders` を、server が組み立てて 60 秒ごと（`server::LESSON_REMINDER_INTERVAL`）に tokio のタスクで呼ぶ（`server::App::lesson_reminders`。`main` が回す）。今の時刻は `Clock` のポートだけから読む。触れる環境で `CLOCK_OFFSET_SECONDS` で時計を進めれば、進めた時刻の窓で作る
2. **窓は domain の `LessonReminderTiming::due_at` の 1 か所で決める**。前日（`day_before`）は開始の 24 時間前（含む）から開始 1 時間前の手前まで、1 時間前（`hour_before`）は開始 1 時間前（含む）から開始の手前まで。窓は重ならない。処理が止まっていて過ぎた窓のリマインドは作らない（1 時間前に確定した予約には前日のリマインドを作らない）
3. **対象は scheduling に問い合わせる**（`LessonReminderTargetLookup`、実装は `scheduling::SchedulingLessonReminderTargetLookup`）。開始が窓に入る枠への予約のうち、問い合わせた時点で確定しているものだけ。キャンセルした予約とコーチのお休みで本部がキャンセルした予約は状態が `cancelled` なので入らず、作る直前に確定のままかを見たことになる。予約はどれもスロット（ビデオ通話の枠）への予約なので、同期レッスンの予約に限られる
4. **お知らせに予約への参照と時点を持たせ、(予約, 種類, 宛先) で一意にする**。`notification.notifications` の種類に `lesson_reminder` を、列に時点 `lesson_reminder_timing` を足し、部分一意索引 `notifications_one_per_lesson_reminder`（`booking_id, lesson_reminder_timing, recipient_id`、`kind = 'lesson_reminder'`）を置く（予約の参照 `booking_id` の列は D108 からある）。種類（前日・1 時間前）は時点の列で表す。挿入は `ON CONFLICT DO NOTHING` で、保存先のポート `NotificationRepository::insert` は新しく残したかを返す。新しく残したときだけプッシュ通知を送る。これまでの `notifications_one_per_push`（種類と予約などの ID の一意）は時点・宛先の違うリマインドを同じとみなすので、リマインドを除いた同じ式の部分一意索引に付け直す
5. **生徒は設定が false なら作らず、コーチには常に作る**。判定は domain の `NotificationSettings::records`（お知らせを残すか）と `allows`（プッシュ通知を送るか）。生徒へのリマインドは `records` が設定に従い、コーチへのリマインド（`PushNotification::CoachLessonReminder`）はどちらも常に true。作らなかったリマインドは、あとで true に戻しても作り直さない
6. **設定の項目を通す**。`getMyNotificationSettings`・`updateMyNotificationSettings` に `lesson_reminder` を足す（`notification.notification_settings` の種類にも足す。行のない種類は既定のオン）。`listMyNotifications` はリマインドに `booking_id` を載せる

## 結果

- 再実行しても重複しない。重複を防ぐのは DB の一意なので、複数の台で処理が回っても 1 件になる。時刻を進める試験（`crates/application/src/notification/send_lesson_reminders.rs`・`crates/interface/tests/lesson_reminders.rs`）で決定論に確かめられる
- マイグレーションは expand（`20261114000001_expand_lesson_reminders.sql`）で、旧版と共存できる。旧版は `lesson_reminder` の行を書かない。ただし旧版の `listMyNotifications` は知らない種類の行を読めないので、リマインドができた後に旧版へ戻すと、リマインドを受け取った人の一覧が障害になる（これまで種類を足したときと同じ）
- プッシュ通知を送れなかったリマインドは、お知らせは残り、送り直さない（at most once）
- 窓の始まりから最大で処理の間隔（60 秒）だけ遅れて作る

## とらなかった案

- 処理の側で送信済みを記録する別の表を持つ: お知らせの表とは別に整合を保つ必要があり、一意の置き場所が 2 つになる
- アプリ側の存在確認だけで重複を防ぐ: 確認と挿入のあいだに並んで回った処理（複数の台・再起動の重なり）が両方挿入しうる
- 予約の確定時に、遅延するイベントを積んでおく: キャンセルや再予約で取り消すのが複雑になる（#1463 のプランの判断）
