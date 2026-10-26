-- live_lesson: 同期レッスン（specs/context-map.yaml の live-lesson）。確定した予約の通話セッションと、終了時に記録する接続品質の生の指標。
-- 指標は行動イベント call.ended の 4 属性と同じ。通話品質スコアは保存しない（式はセマンティック層の 1 か所。ADR 0002 Q1）。
-- 確定した予約にしか開始できないことは domain の LiveLessonSession で判定する（予約は scheduling の持ち物なので外部キーは張らない。ADR 0004 の 3）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA live_lesson;

CREATE TABLE live_lesson.sessions (
    session_id       uuid             PRIMARY KEY DEFAULT gen_random_uuid(),
    -- scheduling.bookings の予約（別のコンテキストなので外部キーにしない）
    booking_id       uuid             NOT NULL,
    student_id       uuid             NOT NULL,
    student_role     text             NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    coach_id         uuid             NOT NULL REFERENCES identity.coaches (coach_id),
    started_at       timestamptz      NOT NULL,
    -- 終了するまでは null（API の LiveLessonSession.status は ended_at の有無で決まる）
    ended_at         timestamptz,
    -- 接続品質の生の指標（終了時に記録する。終了するまでは null）
    -- packet_loss_rate: 失われたパケットの百分率（0〜100）
    packet_loss_rate double precision CHECK (packet_loss_rate BETWEEN 0 AND 100),
    jitter_ms        integer          CHECK (jitter_ms >= 0),
    round_trip_ms    integer          CHECK (round_trip_ms >= 0),
    disconnect_count integer          CHECK (disconnect_count >= 0),
    CONSTRAINT sessions_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role),
    -- 同じ予約のセッションは 1 つ
    CONSTRAINT sessions_booking_id_key UNIQUE (booking_id),
    -- 終了は開始の後（同じ時刻は許す）
    CONSTRAINT sessions_ends_after_starts CHECK (ended_at >= started_at),
    -- 指標は終了したセッションだけが 4 つ揃えて持つ
    CONSTRAINT sessions_quality_when_ended CHECK (
        (ended_at IS NULL AND packet_loss_rate IS NULL AND jitter_ms IS NULL
            AND round_trip_ms IS NULL AND disconnect_count IS NULL) OR
        (ended_at IS NOT NULL AND packet_loss_rate IS NOT NULL AND jitter_ms IS NOT NULL
            AND round_trip_ms IS NOT NULL AND disconnect_count IS NOT NULL)
    )
);

CREATE INDEX sessions_student_id_idx ON live_lesson.sessions (student_id);
CREATE INDEX sessions_coach_id_idx ON live_lesson.sessions (coach_id);
