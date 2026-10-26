-- レッスンの完了記録（glossary の lesson）。完了記録は live-lesson / async-lesson のそれぞれの集約が発行するので、
-- 集約の外に lesson のスキーマを作らず、元の集約と同じスキーマに置く（docs/adr/0006）。
-- is_first_lesson（その生徒の完了記録が 0 件のときだけ true）の判定は domain の LessonCompletion で、2 つのテーブルにまたがる。
-- DB はテーブルごとに「生徒の初回は 1 件」までを守り、テーブルをまたいだ初回の重なりは保存先の実装が生徒ごとの
-- advisory lock で防ぐ（ADR 0004 の 4 の PgSlotRepository と同じ）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

-- 完了記録の生徒・コーチが元の集約の当事者と一致するよう、複合外部キーの参照先を足す
ALTER TABLE live_lesson.sessions
    ADD CONSTRAINT sessions_parties_key UNIQUE (session_id, student_id, coach_id);
ALTER TABLE async_lesson.videos
    ADD CONSTRAINT videos_parties_key UNIQUE (video_id, student_id, coach_id);

-- 同期レッスンの完了記録。元は通話セッション（終了済みかは domain の LiveLessonSession::complete で判定し、DB では守らない）
CREATE TABLE live_lesson.lesson_completions (
    lesson_id       uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id      uuid        NOT NULL,
    student_id      uuid        NOT NULL,
    coach_id        uuid        NOT NULL,
    is_first_lesson boolean     NOT NULL,
    completed_at    timestamptz NOT NULL,
    CONSTRAINT lesson_completions_session_fkey FOREIGN KEY (session_id, student_id, coach_id)
        REFERENCES live_lesson.sessions (session_id, student_id, coach_id),
    -- 同じセッションの完了は 1 回
    CONSTRAINT lesson_completions_session_id_key UNIQUE (session_id)
);

-- 生徒の初回は（このテーブルの中で）1 件
CREATE UNIQUE INDEX lesson_completions_one_first_per_student
    ON live_lesson.lesson_completions (student_id) WHERE is_first_lesson;
CREATE INDEX lesson_completions_student_id_idx ON live_lesson.lesson_completions (student_id);

-- 非同期レッスンの完了記録。元は添削を返した動画（添削の行があることを外部キーで守る）
CREATE TABLE async_lesson.lesson_completions (
    lesson_id       uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    video_id        uuid        NOT NULL REFERENCES async_lesson.video_reviews (video_id),
    student_id      uuid        NOT NULL,
    coach_id        uuid        NOT NULL,
    is_first_lesson boolean     NOT NULL,
    completed_at    timestamptz NOT NULL,
    CONSTRAINT lesson_completions_video_fkey FOREIGN KEY (video_id, student_id, coach_id)
        REFERENCES async_lesson.videos (video_id, student_id, coach_id),
    -- 同じ動画の完了は 1 回
    CONSTRAINT lesson_completions_video_id_key UNIQUE (video_id)
);

-- 生徒の初回は（このテーブルの中で）1 件
CREATE UNIQUE INDEX lesson_completions_one_first_per_student
    ON async_lesson.lesson_completions (student_id) WHERE is_first_lesson;
CREATE INDEX lesson_completions_student_id_idx ON async_lesson.lesson_completions (student_id);
