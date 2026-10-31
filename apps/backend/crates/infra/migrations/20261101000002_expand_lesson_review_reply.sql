-- review: レッスンの評価へのコーチの返信（D131。glossary の lesson_review_reply、domain の LessonReviewReply）。
-- 評価されたコーチが評価 1 件に 1 つだけ付ける短い文と、付けた時刻。評価 1 件に 1 つなので review.lesson_reviews に列を足す。
-- 2 回目は上書き（文と時刻を新しい値に置き換える）で、返信の削除はない。既存の評価と返信のない評価は両方 NULL として読む。
-- 文と時刻は揃って NULL か揃って値を持つ。返信するのが評価されたコーチであること、文が空でないこと（全角空白などを含む
-- 空白の判定）と上限 200 文字の判定は domain の LessonReview::write_reply / LessonReviewReplyComment で行い、DB は壊れた値を
-- 入れないためだけに持つ。
-- 列を足すだけで、既存の列と制約は変えない（古い版のアプリは新しい列を読まず、書くときは NULL のまま）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE review.lesson_reviews
    ADD COLUMN reply_comment text
        CONSTRAINT lesson_reviews_reply_comment_check
            CHECK (btrim(reply_comment) <> '' AND char_length(reply_comment) <= 200),
    ADD COLUMN replied_at timestamptz
        CONSTRAINT lesson_reviews_replied_after_submission CHECK (replied_at >= submitted_at),
    ADD CONSTRAINT lesson_reviews_reply_shape
        CHECK ((reply_comment IS NULL) = (replied_at IS NULL));
