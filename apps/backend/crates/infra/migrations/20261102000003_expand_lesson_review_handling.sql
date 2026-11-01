-- review: 低い評価に本部が付ける「対応済み」の印（D148。glossary の low_rated_review_handling、domain の LowRatedReviewHandling）。
-- 本部が低い評価を確認したかを残し、まだ見ていないものだけを一覧（listLowRatedLessonReviews）で拾えるようにする。
-- 評価 1 件に高々 1 つで、付けた時刻 handled_at と付けた本部のアカウント handled_by を持つ。2 回目は何も変えない（最初の
-- アカウントと時刻を残す。UPDATE ... WHERE handled_at IS NULL）。印を外すことはない。評価 1 件に 1 つなので
-- review.lesson_reviews に列を足す。既存の評価と印のない評価は両方 NULL として読む。時刻とアカウントは揃って NULL か
-- 揃って値を持つ。付けるのは role = 'headquarters' のアカウントだけ（役割つきの複合外部キー。handled_by が NULL なら
-- 外部キーは確かめない）。印を付けられるのが低い点（★2 以下）の評価だけであることは domain の LessonReview::mark_handled /
-- LowRatedReviewCriteria で判定し、DB では守らない（しきい値を 1 か所に置く）。
-- 列を足すだけで、既存の列と制約は変えない（古い版のアプリは新しい列を読まず、評価を書くときは NULL のまま）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE review.lesson_reviews
    ADD COLUMN handled_at timestamptz
        CONSTRAINT lesson_reviews_handled_after_submission CHECK (handled_at >= submitted_at),
    ADD COLUMN handled_by uuid,
    -- 印を付けるのは role = 'headquarters' のアカウントだけ
    ADD COLUMN handled_by_role text NOT NULL DEFAULT 'headquarters'
        CONSTRAINT lesson_reviews_handled_by_role_check CHECK (handled_by_role = 'headquarters'),
    ADD CONSTRAINT lesson_reviews_handled_by_fkey FOREIGN KEY (handled_by, handled_by_role)
        REFERENCES identity.accounts (account_id, role),
    ADD CONSTRAINT lesson_reviews_handling_shape
        CHECK ((handled_at IS NULL) = (handled_by IS NULL));
