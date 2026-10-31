-- notification: 本部からのお知らせ（D124 / Issue #382）。本部が生徒全員か、あるスポーツの生徒全員に送ったお知らせの記録
-- （domain の Announcement）を notification.announcements に残し、受け取る生徒ごとにお知らせ（notification.notifications）
-- を 1 件作る（domain の PushNotification::Announcement と NotificationKind::Announcement）。
-- 本部からのお知らせのお知らせは生徒だけが当事者で、コーチを持たない（coach_id は NULL）。中身は送った記録（announcement_id）
-- だけを持つ。題と本文は送った記録にあり、お知らせには写さない。受け取り設定の種類にも同じ値を足す（受け取り設定は
-- すべての種類の行を書くため）。
-- どれも表・列を足すか、許す値・形を広げるだけで、既存の行と古い版のアプリの読み書きはそのまま通る（ADR 0004 の 2 の緩和）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE notification.announcements (
    announcement_id uuid        PRIMARY KEY,
    -- 残した順（同じ時刻の記録を後に残したものから並べる）
    seq             bigint      GENERATED ALWAYS AS IDENTITY,
    -- 送った本部のアカウント（役割は境界が確かめる）
    sender_id       uuid        NOT NULL REFERENCES identity.accounts (account_id),
    -- 題（1〜50 文字。空白だけは不可）と本文（1〜2000 文字。空白だけは不可）。上限は domain の AnnouncementTitle /
    -- AnnouncementBody と契約の maxLength
    title           text        NOT NULL
        CHECK (btrim(title) <> '' AND char_length(title) <= 50),
    body            text        NOT NULL
        CHECK (btrim(body) <> '' AND char_length(body) <= 2000),
    -- 対象のスポーツ（identity.accounts.sport と同じ値）。生徒全員に送ったなら NULL
    sport           text
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming')),
    -- 受け取った生徒の数（受け取る生徒がいなければ送らない）
    recipient_count integer     NOT NULL CHECK (recipient_count >= 1),
    sent_at         timestamptz NOT NULL
);

-- 送った記録を新しい順に読む
CREATE INDEX announcements_sent_at
    ON notification.announcements (sent_at DESC, seq DESC);

ALTER TABLE notification.notification_settings
    DROP CONSTRAINT notification_settings_kind_check,
    ADD CONSTRAINT notification_settings_kind_check
        CHECK (kind IN ('booking_confirmed', 'video_review_requested', 'video_review_returned', 'message_received',
                        'announcement'));

-- 本部からのお知らせのお知らせは送った記録を指す（同じコンテキストの表なので外部キーを張る）
ALTER TABLE notification.notifications
    ADD COLUMN announcement_id uuid REFERENCES notification.announcements (announcement_id);

-- 本部からのお知らせのお知らせはコーチを持たない。ほかの種類は notifications_kind_shape で coach_id を必須のままにする
ALTER TABLE notification.notifications
    ALTER COLUMN coach_id DROP NOT NULL;

ALTER TABLE notification.notifications
    DROP CONSTRAINT notifications_kind_check,
    ADD CONSTRAINT notifications_kind_check
        CHECK (kind IN ('booking_confirmed', 'booking_received', 'video_review_requested', 'video_review_returned',
                        'message_received', 'announcement')),
    -- relax: announcement の形を OR で足す。前の 4 つの形に足した coach_id IS NOT NULL と announcement_id IS NULL は、
    -- 前の定義の列の NOT NULL と、この版で足した列（既存の行と古い版のアプリでは NULL）のことで、既存の行はすべて通る
    DROP CONSTRAINT notifications_kind_shape,
    ADD CONSTRAINT notifications_kind_shape CHECK (
        (kind IN ('booking_confirmed', 'booking_received')
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

-- 同じ本部からのお知らせは 1 人の生徒に 1 件（notifications_one_per_push は予約・添削・メッセージの ID で数えるので、
-- 送った記録だけを持つお知らせはここで守る）
CREATE UNIQUE INDEX notifications_one_per_announcement
    ON notification.notifications (announcement_id, recipient_id)
    WHERE kind = 'announcement';
