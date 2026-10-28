-- async_lesson: 動画への AI の一次フィードバック（domain の AiFeedback。変更シナリオ cs-006）。
-- 生徒が動画を送った直後に AI（Gen1 はモック）が返す短い文と、それを作った時刻。動画 1 本に 1 つなので async_lesson.videos に列を足す。
-- コーチの添削（async_lesson.video_reviews）とは別の列で、添削の返却では書き換えない。添削の着手条件（不変条件 2）にも関わらない。
-- AI が失敗したら送付は成功させて両方 NULL のまま（既存の動画も NULL として読む）。文と時刻は揃って NULL か揃って値を持つ。
-- 文が空でないこと（全角空白などを含む空白の判定）と、作ったのが送付の後であることの判定は domain で行い、DB は壊れた値を入れないためだけに持つ。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE async_lesson.videos
    ADD COLUMN ai_feedback_comment text
        CONSTRAINT videos_ai_feedback_comment_check CHECK (btrim(ai_feedback_comment) <> ''),
    ADD COLUMN ai_feedback_created_at timestamptz
        CONSTRAINT videos_ai_feedback_after_submission CHECK (ai_feedback_created_at >= submitted_at),
    ADD CONSTRAINT videos_ai_feedback_shape
        CHECK ((ai_feedback_comment IS NULL) = (ai_feedback_created_at IS NULL));
