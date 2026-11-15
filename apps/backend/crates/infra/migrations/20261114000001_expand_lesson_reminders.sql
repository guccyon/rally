-- notification: レッスンのリマインド（Issue #1476 / #1463。domain の PushNotification::LessonReminder・CoachLessonReminder と
-- NotificationKind::LessonReminder、判断は docs/adr/0013-lesson-reminder-once-per-booking-timing-recipient.md）。確定した
-- 同期レッスンの予約について、開始の前日と開始 1 時間前（domain の LessonReminderTiming）に、生徒とコーチへのお知らせを
-- 定期の処理が作る。同じ窓で処理が何度回っても重複しないことは、ここの一意で守る。
--
-- 1. notification.notifications の種類（kind）に lesson_reminder を足す。生徒へもコーチへも同じ値で、どちらへかは受け取る人
--    （recipient_id）で分かる。中身は予約（booking_id。お知らせから予約を開く参照。列は D108 からある）だけで、形は予約の
--    確定と同じ。
-- 2. notification.notifications に、リマインドの時点 lesson_reminder_timing（day_before / hour_before。値の対応は infra の
--    notification_values）を足す。リマインドだけが持ち、ほかの種類は NULL（notifications_lesson_reminder_timing_shape）。
-- 3. リマインドは (予約, 時点, 宛先) で 1 件（notifications_one_per_lesson_reminder）。これまでの notifications_one_per_push
--    （種類と予約・添削・メッセージの ID）は、同じ予約の時点・宛先の違うリマインドも同じとみなすので、リマインドを除いた
--    同じ式の部分一意索引（notifications_one_per_push_except_lesson_reminder）に置き換える。
-- 4. notification.notification_settings の種類に lesson_reminder を足す（生徒がリマインドを切る設定。行のない種類は既定の
--    オンとして読むので、既存のアカウントの行は足さない）。
--
-- どれも列・索引・制約を足すか、許す値・形を広げるか、一意を狭く付け直すだけで、既存の行と古い版のアプリの書き込みは
-- そのまま通る（ADR 0004 の 2 の緩和）。古い版のアプリは lesson_reminder の行を書かない。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE notification.notifications
    ADD COLUMN lesson_reminder_timing text CHECK (lesson_reminder_timing IN ('day_before', 'hour_before'));

ALTER TABLE notification.notifications
    DROP CONSTRAINT notifications_kind_check,
    ADD CONSTRAINT notifications_kind_check
        CHECK (kind IN ('booking_confirmed', 'booking_received', 'video_review_requested', 'video_review_returned',
                        'message_received', 'announcement', 'booking_cancelled_by_headquarters', 'lesson_reminder')),
    -- relax: 前の定義の 5 つの形はそのままで、lesson_reminder を予約の確定と同じ形に足すだけ（既存の行はすべて通る）
    DROP CONSTRAINT notifications_kind_shape,
    ADD CONSTRAINT notifications_kind_shape CHECK (
        (kind IN ('booking_confirmed', 'booking_received', 'booking_cancelled_by_headquarters', 'lesson_reminder')
            AND coach_id IS NOT NULL
            AND booking_id IS NOT NULL
            AND video_id IS NULL AND review_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL
            AND announcement_id IS NULL)
        OR (kind = 'video_review_requested'
            AND coach_id IS NOT NULL
            AND video_id IS NOT NULL
            AND booking_id IS NULL AND review_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL
            AND announcement_id IS NULL)
        OR (kind = 'video_review_returned'
            AND coach_id IS NOT NULL
            AND video_id IS NOT NULL AND review_id IS NOT NULL
            AND booking_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL
            AND announcement_id IS NULL)
        OR (kind = 'message_received'
            AND coach_id IS NOT NULL
            AND thread_id IS NOT NULL AND message_id IS NOT NULL AND sender_role IS NOT NULL
            AND booking_id IS NULL AND video_id IS NULL AND review_id IS NULL
            AND announcement_id IS NULL)
        OR (kind = 'announcement'
            AND announcement_id IS NOT NULL
            AND recipient_id = student_id AND coach_id IS NULL
            AND booking_id IS NULL AND video_id IS NULL AND review_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL)
    );

-- 時点を持つのはリマインドだけで、リマインドは必ず時点を持つ（既存の行は lesson_reminder でなく、時点は NULL なので通る）
ALTER TABLE notification.notifications
    ADD CONSTRAINT notifications_lesson_reminder_timing_shape
        CHECK ((kind = 'lesson_reminder') = (lesson_reminder_timing IS NOT NULL));

-- リマインドは同じ予約・同じ時点・同じ宛先に 1 件（定期の処理が同じ窓で何度回っても重複しない。挿入は ON CONFLICT DO NOTHING）
CREATE UNIQUE INDEX notifications_one_per_lesson_reminder
    ON notification.notifications (booking_id, lesson_reminder_timing, recipient_id)
    WHERE kind = 'lesson_reminder';

-- リマインドのほかは、これまでと同じ式で同じプッシュ通知を 1 件にする
CREATE UNIQUE INDEX notifications_one_per_push_except_lesson_reminder
    ON notification.notifications (kind, COALESCE(booking_id, review_id, message_id))
    WHERE kind <> 'lesson_reminder';

-- relax: 直前の notifications_one_per_push_except_lesson_reminder が、リマインドを除くすべての行で同じ式の一意を守る（既存の行に
-- lesson_reminder はなく、古い版のアプリも書かないので、古い版の書き込みが受ける一意はこれまでと変わらない）
DROP INDEX notification.notifications_one_per_push;

ALTER TABLE notification.notification_settings
    DROP CONSTRAINT notification_settings_kind_check,
    ADD CONSTRAINT notification_settings_kind_check
        CHECK (kind IN ('booking_confirmed', 'video_review_requested', 'video_review_returned', 'message_received',
                        'announcement', 'booking_cancelled_by_headquarters', 'lesson_reminder'));
