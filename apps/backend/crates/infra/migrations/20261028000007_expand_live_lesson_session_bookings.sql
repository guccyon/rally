-- live_lesson: グループレッスンの通話（変更シナリオ cs-004「グループレッスン（1 コーチ : N 生徒）を追加する」の 2 本目。D41）。
-- 定員 2 以上の枠では、通話を始めたときに確定していた同じ枠の予約をすべて 1 つの通話セッションに結び付け
-- （domain の LiveLessonSession::start と SessionBooking）、完了したレッスンは結び付いた予約（生徒）ごとに 1 つ発行する。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

-- 通話セッションに結び付いた予約。マンツーマンのセッションは 1 行、グループレッスンのセッションは人数分。
-- sessions.booking_id / student_id は開始した予約として今までどおり書き、その予約もここに 1 行持つ。
-- この表の行がないセッション（この版より前に始めたもの）は、開始した予約だけが結び付いたものとして読む。
-- 予約は scheduling の持ち物なので booking_id に外部キーは張らない（ADR 0004 の 3）
CREATE TABLE live_lesson.session_bookings (
    session_id   uuid NOT NULL REFERENCES live_lesson.sessions (session_id),
    booking_id   uuid NOT NULL,
    student_id   uuid NOT NULL,
    student_role text NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    -- 結び付いた順（0 は開始した予約。domain の LiveLessonSession::bookings の並び）
    position     smallint NOT NULL CHECK (position >= 0),
    PRIMARY KEY (session_id, booking_id),
    CONSTRAINT session_bookings_session_position_key UNIQUE (session_id, position),
    CONSTRAINT session_bookings_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role),
    -- 1 つの予約が結び付くセッションは 1 つ（並行な開始が両方保存されないように）
    CONSTRAINT session_bookings_booking_id_key UNIQUE (booking_id),
    -- 同じ生徒は 1 つのセッションに 1 回（同じ枠の有効な予約は生徒ごとに 1 件。D40）
    CONSTRAINT session_bookings_session_student_key UNIQUE (session_id, student_id)
);

-- 完了記録のコーチがセッションのコーチと一致するよう、複合外部キーの参照先を足す
ALTER TABLE live_lesson.sessions
    ADD CONSTRAINT sessions_session_coach_key UNIQUE (session_id, coach_id);

-- 同期レッスンの完了記録は、同じセッションでも生徒ごとに 1 回（グループレッスン）。コーチはセッションのコーチ
ALTER TABLE live_lesson.lesson_completions
    ADD CONSTRAINT lesson_completions_session_student_key UNIQUE (session_id, student_id),
    ADD CONSTRAINT lesson_completions_session_coach_fkey FOREIGN KEY (session_id, coach_id)
        REFERENCES live_lesson.sessions (session_id, coach_id);

-- セッション 1 つにつき完了 1 回、の一意の制約を外す（上の生徒ごとの一意より厳しい。一意性の緩和で、データは消えない）。
-- 生徒・コーチがセッションの（開始した予約の）当事者、の複合外部キーも外す（グループレッスンの 2 人目以降の生徒は
-- sessions.student_id にいない）。生徒が結び付いた予約の生徒であることは domain の LiveLessonSession::complete_for で判定し、
-- コーチは上の lesson_completions_session_coach_fkey が守る。古い版のアプリはマンツーマンの完了だけを書き、
-- それは新しい制約も満たす（ADR 0004 の 2 の緩和）。
-- relax: セッション 1 つにつき完了 1 回を、生徒ごとに 1 回（lesson_completions_session_student_key）に緩める
ALTER TABLE live_lesson.lesson_completions DROP CONSTRAINT lesson_completions_session_id_key;
-- relax: グループレッスンの 2 人目以降の生徒は sessions.student_id にいない（コーチは lesson_completions_session_coach_fkey が守る）
ALTER TABLE live_lesson.lesson_completions DROP CONSTRAINT lesson_completions_session_fkey;
