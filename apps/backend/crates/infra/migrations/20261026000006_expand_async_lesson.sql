-- async_lesson: 非同期レッスン（specs/context-map.yaml の async-lesson）。生徒がコーチに送った動画と、コーチが返した添削。
-- 動画本体は保存先（S3 への直接アップロード。Gen1 はモック）に置き、ここには持たない。
-- 動画の状態（API の VideoReview.status）は添削の行の有無で決まるので、状態の列は持たない。
-- ゴルフの添削はワンショット決済が確定した動画にしか着手できない（不変条件 2）ことは domain の ContractType::may_start_review で判定する
-- （決済は billing の持ち物で、Gen1 は billing のテーブルがない。ADR 0004 の 4）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA async_lesson;

CREATE TABLE async_lesson.videos (
    video_id     uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id   uuid        NOT NULL,
    student_role text        NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    coach_id     uuid        NOT NULL REFERENCES identity.coaches (coach_id),
    sport        text        NOT NULL CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball')),
    submitted_at timestamptz NOT NULL,
    CONSTRAINT videos_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role)
);

CREATE INDEX videos_student_id_idx ON async_lesson.videos (student_id);
CREATE INDEX videos_coach_id_submitted_at_idx ON async_lesson.videos (coach_id, submitted_at);

CREATE TABLE async_lesson.video_reviews (
    review_id   uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    video_id    uuid        NOT NULL REFERENCES async_lesson.videos (video_id),
    -- コーチの添削コメント。空と半角空白だけは入らない（全角空白などを含む空白の判定と上限 4000 文字は domain の ReviewComment）
    comment     text        NOT NULL CHECK (btrim(comment) <> ''),
    returned_at timestamptz NOT NULL,
    -- 動画 1 本の添削は 1 つ（返却は 1 回）
    CONSTRAINT video_reviews_video_id_key UNIQUE (video_id)
);
