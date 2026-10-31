-- notification: コーチへの添削の依頼の通知（D114 / Issue #367）。生徒が動画を添削依頼として送った（async-lesson の
-- VideoReviewRequested）ことを、依頼を受けたコーチにプッシュ通知で知らせ、お知らせとして残す（domain の
-- PushNotification::VideoReviewRequested と NotificationKind::VideoReviewRequested）。
-- お知らせの種類（kind）に video_review_requested を足し、中身は動画（video_id）だけを持つ（予約・添削・スレッド・
-- メッセージ・送り手は持たない）。受け取り設定の種類にも同じ値を足す（受け取り設定はすべての種類の行を書くため）。
-- どれも許す値・形を広げるだけで、既存の行と古い版のアプリの読み書きはそのまま通る（ADR 0004 の 2 の緩和）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE notification.notification_settings
    DROP CONSTRAINT notification_settings_kind_check,
    ADD CONSTRAINT notification_settings_kind_check
        CHECK (kind IN ('booking_confirmed', 'video_review_requested', 'video_review_returned', 'message_received'));

ALTER TABLE notification.notifications
    DROP CONSTRAINT notifications_kind_check,
    ADD CONSTRAINT notifications_kind_check
        CHECK (kind IN ('booking_confirmed', 'booking_received', 'video_review_requested', 'video_review_returned',
                        'message_received')),
    -- relax: 前の定義の 3 つの形はそのままで、video_review_requested の形を OR で足すだけ（既存の行はすべて通る）
    DROP CONSTRAINT notifications_kind_shape,
    ADD CONSTRAINT notifications_kind_shape CHECK (
        (kind IN ('booking_confirmed', 'booking_received')
            AND booking_id IS NOT NULL
            AND video_id IS NULL AND review_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL)
        OR (kind = 'video_review_requested'
            AND video_id IS NOT NULL
            AND booking_id IS NULL AND review_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL)
        OR (kind = 'video_review_returned'
            AND video_id IS NOT NULL AND review_id IS NOT NULL
            AND booking_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL)
        OR (kind = 'message_received'
            AND thread_id IS NOT NULL AND message_id IS NOT NULL AND sender_role IS NOT NULL
            AND booking_id IS NULL AND video_id IS NULL AND review_id IS NULL)
    );

-- 同じ動画の添削の依頼のお知らせは 1 件（notifications_one_per_push は予約・添削・メッセージの ID で数えるので、
-- 動画だけを持つ依頼はここで守る）
CREATE UNIQUE INDEX notifications_one_per_video_review_request
    ON notification.notifications (video_id)
    WHERE kind = 'video_review_requested';
