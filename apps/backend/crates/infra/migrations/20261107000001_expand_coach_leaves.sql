-- scheduling・notification: コーチのお休み（Issue #455。domain の CoachLeave）。本部がコーチのお休みの期間を決め、その期間の
-- 予約をまとめてキャンセルし（キャンセルした人は本部）、期間の枠をまとめて取り下げた記録と、予約した生徒へのお知らせ。
--
-- 1. scheduling.bookings に、予約をキャンセルした人 cancelled_by（domain の BookingCanceller。値の対応は infra の
--    scheduling_values）を足す。キャンセル済みの予約だけが持ち、NULL はこの列を足す前のキャンセル（キャンセルできたのは
--    予約した生徒だけなので、新しい版のアプリは生徒として読む）と、古い版のアプリが書いたキャンセル（同じく生徒だけ）。
-- 2. scheduling.coach_leaves に、お休み 1 回につき 1 行（期間・生徒への一言・操作した本部のアカウント・時刻・取り下げた枠の
--    数）を残す。scheduling.coach_leave_bookings に、そのお休みでキャンセルした予約 1 件につき 1 行（生徒と、知らせる枠の
--    日時の写し）を残す。予約・枠の変更と同じトランザクションで書き、書いた行は書き換えない（取り消しはない）。操作したのは
--    role = 'headquarters' のアカウントだけ（役割つきの外部キー）。同じコーチの期間の重なるお休みを持たないことと、期間の
--    規則（31 日まで・始まりは明日以降）は domain の CoachLeave の 1 か所で判定し、ここでは長さと向きだけを守る。
-- 3. notification.notifications の種類（kind）に booking_cancelled_by_headquarters（domain の
--    PushNotification::BookingCancelledByHeadquarters。本部が予約をキャンセルしたことを生徒に知らせる）を足す。中身は予約
--    （booking_id）だけで、形は予約の確定と同じ。受け取り設定の種類は予約の確定（booking_confirmed）に入れるので足さない。
--
-- どれも表・列を足すか、許す値・形を広げるだけで、既存の行と古い版のアプリの読み書きはそのまま通る（ADR 0004 の 2 の緩和）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE scheduling.bookings
    ADD COLUMN cancelled_by text CHECK (cancelled_by IN ('student', 'coach', 'headquarters'));

ALTER TABLE scheduling.bookings
    ADD CONSTRAINT bookings_cancelled_by_only_when_cancelled
        CHECK (cancelled_by IS NULL OR status = 'cancelled');

CREATE TABLE scheduling.coach_leaves (
    coach_leave_id       uuid        PRIMARY KEY,
    -- 保存した順（同じ時刻の記録の並べ方に使う）
    seq                  bigint      GENERATED ALWAYS AS IDENTITY,
    coach_id             uuid        NOT NULL REFERENCES identity.coaches (coach_id),
    -- お休みの期間（日本時間の始まりの日の 0 時から、終わりの日の翌日の 0 時まで。終わりは含まない）
    starts_at            timestamptz NOT NULL,
    ends_at              timestamptz NOT NULL,
    -- 生徒への一言（0〜200 文字。書いたとおり。上限は domain の CoachLeaveNote と契約の maxLength）
    note                 text        NOT NULL CHECK (char_length(note) <= 200),
    created_by           uuid        NOT NULL,
    -- お休みにするのは role = 'headquarters' のアカウントだけ
    created_by_role      text        NOT NULL DEFAULT 'headquarters' CHECK (created_by_role = 'headquarters'),
    created_at           timestamptz NOT NULL,
    -- 取り下げた枠の数（予約のあった枠を含む）
    withdrawn_slot_count integer     NOT NULL CHECK (withdrawn_slot_count >= 0),
    CONSTRAINT coach_leaves_ends_after_starts CHECK (ends_at > starts_at),
    CONSTRAINT coach_leaves_created_by_fkey FOREIGN KEY (created_by, created_by_role)
        REFERENCES identity.accounts (account_id, role)
);

-- コーチ 1 人のお休みを期間で読む（重なりの判定）
CREATE INDEX coach_leaves_coach_id_starts_at_idx
    ON scheduling.coach_leaves (coach_id, starts_at);

CREATE TABLE scheduling.coach_leave_bookings (
    -- 予約は 1 回しかキャンセルできないので、1 件の予約は高々 1 つのお休みに入る
    booking_id     uuid        PRIMARY KEY REFERENCES scheduling.bookings (booking_id),
    coach_leave_id uuid        NOT NULL REFERENCES scheduling.coach_leaves (coach_leave_id),
    student_id     uuid        NOT NULL REFERENCES identity.accounts (account_id),
    -- 予約した枠の開始と終了の写し（生徒へのお知らせの日時）
    starts_at      timestamptz NOT NULL,
    ends_at        timestamptz NOT NULL,
    CONSTRAINT coach_leave_bookings_ends_after_starts CHECK (ends_at > starts_at)
);

CREATE INDEX coach_leave_bookings_coach_leave_id_idx
    ON scheduling.coach_leave_bookings (coach_leave_id);

ALTER TABLE notification.notifications
    DROP CONSTRAINT notifications_kind_check,
    ADD CONSTRAINT notifications_kind_check
        CHECK (kind IN ('booking_confirmed', 'booking_received', 'video_review_requested', 'video_review_returned',
                        'message_received', 'announcement', 'booking_cancelled_by_headquarters')),
    -- relax: 前の定義の 5 つの形はそのままで、booking_cancelled_by_headquarters を予約の確定と同じ形に足すだけ（既存の行は
    -- すべて通る）
    DROP CONSTRAINT notifications_kind_shape,
    ADD CONSTRAINT notifications_kind_shape CHECK (
        (kind IN ('booking_confirmed', 'booking_received', 'booking_cancelled_by_headquarters')
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
