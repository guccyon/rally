-- notification: 本部による予約のキャンセルの受け取り設定（Issue #991）。notification.notification_settings の種類（kind）に
-- booking_cancelled_by_headquarters（domain の NotificationKind::BookingCancelledByHeadquarters。コーチのお休みで本部が
-- 予約をキャンセルしたことの生徒への知らせ）を足す。これまでこの知らせは予約の確定（booking_confirmed）の設定に従っていたが、
-- 自分の種類の設定に従うようにする。行のない種類は既定のオンとして読むので、既存のアカウントの行は足さない（予約の確定を
-- オフにしていた生徒にも、キャンセルの知らせは届くようになる）。
--
-- 許す値を広げるだけで、既存の行と古い版のアプリの読み書きはそのまま通る（ADR 0004 の 2 の緩和）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE notification.notification_settings
    DROP CONSTRAINT notification_settings_kind_check,
    ADD CONSTRAINT notification_settings_kind_check
        CHECK (kind IN ('booking_confirmed', 'video_review_requested', 'video_review_returned', 'message_received',
                        'announcement', 'booking_cancelled_by_headquarters'));
