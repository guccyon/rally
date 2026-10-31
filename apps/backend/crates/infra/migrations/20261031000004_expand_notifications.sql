-- notification: お知らせ（D108）。送ったプッシュ通知（domain の PushNotification）を、受け取る人があとで一覧で見られるように
-- 残す（domain の Notification）。受け取り設定（notification.notification_settings）でオフの種類も残す（オフが止めるのは
-- プッシュ通知だけ）。通知 1 件につき 1 行で、中身はプッシュ通知の種類（kind）ごとに、予約（booking_id）・動画と添削
-- （video_id / review_id）・スレッドとメッセージと送り手（thread_id / message_id / sender_role）を持つ（notifications_kind_shape）。
-- 予約・動画・添削・スレッド・メッセージは別のコンテキストの持ち物なので外部キーは張らない（ADR 0004 の 3）。受け取る人
-- （recipient_id）は domain の PushNotification::recipient を infra が書き、ここではどちらかの当事者であることだけを守る。
-- 本文（表示する文言）は保存せず、境界のアダプタが種類と中身から決める。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE notification.notifications (
    notification_id uuid        PRIMARY KEY,
    -- 残した順（同じ時刻のお知らせを後に残したものから並べる）
    seq             bigint      GENERATED ALWAYS AS IDENTITY,
    recipient_id    uuid        NOT NULL REFERENCES identity.accounts (account_id),
    -- プッシュ通知の種類（domain の PushNotification の変種。値の対応は infra の notification_values）。
    -- booking_confirmed は生徒への予約の確定、booking_received はコーチへの新しい予約
    kind            text        NOT NULL
        CHECK (kind IN ('booking_confirmed', 'booking_received', 'video_review_returned', 'message_received')),
    student_id      uuid        NOT NULL REFERENCES identity.accounts (account_id),
    coach_id        uuid        NOT NULL REFERENCES identity.accounts (account_id),
    booking_id      uuid,
    video_id        uuid,
    review_id       uuid,
    thread_id       uuid,
    message_id      uuid,
    -- メッセージの送り手がスレッドのどちらの当事者か（messaging.messages.sender_role の当事者の値と同じ）
    sender_role     text        CHECK (sender_role IN ('student', 'coach')),
    created_at      timestamptz NOT NULL,
    -- 受け取る人が読んだ時刻（読むまでは NULL）
    read_at         timestamptz,
    CONSTRAINT notifications_recipient_is_a_party
        CHECK (recipient_id IN (student_id, coach_id)),
    CONSTRAINT notifications_kind_shape CHECK (
        (kind IN ('booking_confirmed', 'booking_received')
            AND booking_id IS NOT NULL
            AND video_id IS NULL AND review_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL)
        OR (kind = 'video_review_returned'
            AND video_id IS NOT NULL AND review_id IS NOT NULL
            AND booking_id IS NULL
            AND thread_id IS NULL AND message_id IS NULL AND sender_role IS NULL)
        OR (kind = 'message_received'
            AND thread_id IS NOT NULL AND message_id IS NOT NULL AND sender_role IS NOT NULL
            AND booking_id IS NULL AND video_id IS NULL AND review_id IS NULL)
    ),
    CONSTRAINT notifications_read_after_created CHECK (read_at IS NULL OR read_at >= created_at)
);

-- 同じプッシュ通知のお知らせは 1 件（境界間イベントの再送や送り直しで 2 件にしない）。予約・添削・メッセージのどれか 1 つと
-- 種類でプッシュ通知が決まる
CREATE UNIQUE INDEX notifications_one_per_push
    ON notification.notifications (kind, COALESCE(booking_id, review_id, message_id));

-- 受け取る人のお知らせを新しい順に読む
CREATE INDEX notifications_recipient_created_at
    ON notification.notifications (recipient_id, created_at DESC, seq DESC);
