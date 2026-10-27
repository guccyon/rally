-- messaging: メッセージ（specs/context-map.yaml の messaging）。生徒と担当コーチの 1 対 1 のチャットのスレッドと、メッセージ。
-- スレッドは予約の確定（境界間イベント BookingConfirmed）で messaging が開く。予約は scheduling の持ち物なので外部キーは張らない（ADR 0004 の 3）。
-- 月額の生徒はチャットが送り放題（送信の回数は数えず、保存もしない）。生徒が送ってよいかは domain の
-- Entitlement::unlimited_video_and_chat で判定し、DB では守らない（entitlement の持ち物。ADR 0004 の 4）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA messaging;

CREATE TABLE messaging.threads (
    thread_id    uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id   uuid        NOT NULL,
    student_role text        NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    coach_id     uuid        NOT NULL REFERENCES identity.coaches (coach_id),
    opened_at    timestamptz NOT NULL,
    CONSTRAINT threads_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role),
    -- 同じ生徒とコーチのスレッドは 1 つ
    CONSTRAINT threads_student_id_coach_id_key UNIQUE (student_id, coach_id)
);

CREATE INDEX threads_coach_id_idx ON messaging.threads (coach_id);

CREATE TABLE messaging.messages (
    message_id  uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id   uuid        NOT NULL REFERENCES messaging.threads (thread_id),
    -- 送った当事者。student: スレッドの生徒 / coach: スレッドの担当コーチ（アカウントはスレッドの列から決まる）
    sender_role text        NOT NULL CHECK (sender_role IN ('student', 'coach')),
    -- 本文。空と半角空白だけは入らない（全角空白などを含む空白の判定と上限 2000 文字は domain の MessageBody）
    body        text        NOT NULL CHECK (btrim(body) <> ''),
    sent_at     timestamptz NOT NULL,
    -- 受け取った側が読んだ時刻（既読）。読むまでは NULL
    read_at     timestamptz,
    -- スレッドの中の送った順（同じ送信時刻のメッセージの並びを保つ）
    seq         bigint      GENERATED ALWAYS AS IDENTITY,
    CONSTRAINT messages_read_after_sent CHECK (read_at IS NULL OR read_at >= sent_at)
);

CREATE INDEX messages_thread_id_seq_idx ON messaging.messages (thread_id, seq);
