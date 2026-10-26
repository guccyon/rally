-- scheduling: 予約（specs/context-map.yaml）。スロット（コーチが公開する予約枠）と予約（スロットの確保。体験予約を含む）。
-- キャンセル規定（開始 24 時間前以降のキャンセルは回数を消費）の判定はアプリケーション（scheduling。ADR 0002）に置き、DB の制約にしない。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA scheduling;

CREATE TABLE scheduling.slots (
    slot_id    uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    coach_id   uuid        NOT NULL REFERENCES identity.coaches (coach_id),
    sport      text        NOT NULL CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball')),
    starts_at  timestamptz NOT NULL,
    ends_at    timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT slots_ends_after_starts CHECK (ends_at > starts_at)
);

CREATE INDEX slots_sport_starts_at_idx ON scheduling.slots (sport, starts_at);
CREATE INDEX slots_coach_id_starts_at_idx ON scheduling.slots (coach_id, starts_at);

CREATE TABLE scheduling.bookings (
    booking_id    uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    slot_id       uuid        NOT NULL REFERENCES scheduling.slots (slot_id),
    student_id    uuid        NOT NULL,
    -- 予約できるのは role = 'student' のアカウントだけ
    student_role  text        NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    is_trial      boolean     NOT NULL,
    status        text        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'confirmed', 'cancelled')),
    cancel_reason text        CHECK (char_length(cancel_reason) <= 500),
    created_at    timestamptz NOT NULL DEFAULT now(),
    confirmed_at  timestamptz,
    cancelled_at  timestamptz,
    CONSTRAINT bookings_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role),
    -- 状態と時刻の整合（pending からのキャンセルでは confirmed_at は null のまま）
    CONSTRAINT bookings_status_timestamps CHECK (
        (status = 'pending'   AND confirmed_at IS NULL     AND cancelled_at IS NULL) OR
        (status = 'confirmed' AND confirmed_at IS NOT NULL AND cancelled_at IS NULL) OR
        (status = 'cancelled' AND cancelled_at IS NOT NULL)
    ),
    CONSTRAINT bookings_cancel_reason_only_when_cancelled CHECK (cancel_reason IS NULL OR status = 'cancelled')
);

-- 1 つのスロットに有効な（キャンセルされていない）予約は 1 件まで。二重予約を DB で防ぐ
CREATE UNIQUE INDEX bookings_one_active_per_slot ON scheduling.bookings (slot_id) WHERE status <> 'cancelled';
CREATE INDEX bookings_student_id_idx ON scheduling.bookings (student_id);
