-- review: 評価（specs/context-map.yaml の review）。生徒が完了したレッスン（glossary の lesson_completion）に付けるレッスンの評価
-- （glossary の lesson_review）。完了したレッスン 1 件につき 1 つで、評価されるのはそのレッスンのコーチ。
-- 完了したレッスンは live-lesson / async-lesson の持ち物（docs/adr/0006）なので lesson_id に外部キーは張らない（ADR 0004 の 3）。
-- 評価するのがそのレッスンの生徒であることは domain の LessonReview::submit で判定する。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA review;

CREATE TABLE review.lesson_reviews (
    review_id    uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    lesson_id    uuid        NOT NULL,
    student_id   uuid        NOT NULL,
    student_role text        NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    coach_id     uuid        NOT NULL REFERENCES identity.coaches (coach_id),
    -- 1〜5 の点（domain の Rating）
    rating       smallint    NOT NULL CHECK (rating BETWEEN 1 AND 5),
    -- 一言。空と半角空白だけは入らない（全角空白などを含む空白の判定と上限 200 文字は domain の LessonReviewComment）
    comment      text        NOT NULL CHECK (btrim(comment) <> ''),
    submitted_at timestamptz NOT NULL,
    CONSTRAINT lesson_reviews_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role),
    -- 完了したレッスン 1 件の評価は 1 つ（二重の評価の防止）
    CONSTRAINT lesson_reviews_lesson_id_key UNIQUE (lesson_id)
);

-- コーチごとの評価の件数と平均（admin-web の getCoachReviewSummary）はコーチで読む
CREATE INDEX lesson_reviews_coach_id_idx ON review.lesson_reviews (coach_id);
