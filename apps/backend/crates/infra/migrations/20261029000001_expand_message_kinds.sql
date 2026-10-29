-- messaging: メッセージに種類と参照を足す（D47 / Issue #207。docs/adr/0007 の決定 7）。チャットは会話でありレッスンのログでもあり
-- （docs/adr/0009 の答え合わせ）、生徒の添削依頼とコーチの添削の返却がスレッドに流れる。
-- 種類（kind）: text（本文）/ video_review_request（添削依頼）/ video_review_return（添削の返却）。既存のメッセージは text（既定値）。
-- 参照は async-lesson の動画（video_id）と添削（review_id）の ID だけで、添削の有無・回数・状態の写しは持たない。動画・添削は
-- async-lesson の持ち物なので外部キーは張らない（ADR 0004 の 3）。
-- 本文を持つのは text だけなので body の NOT NULL を外す（緩和）。text の本文が空でないことは messages_body_check のまま。
-- 送り手（sender_role）に system（システムが流したもの。まだ書く処理はない）を足す（許す値を広げるだけの付け直し）。
-- 種類ごとの形と送り手（添削依頼は生徒、添削の返却はコーチ）の判定は domain の MessageContent で、DB は壊れた値を入れないために守る。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE messaging.messages
    ADD COLUMN kind text NOT NULL DEFAULT 'text'
        CONSTRAINT messages_kind_check
        CHECK (kind IN ('text', 'video_review_request', 'video_review_return')),
    ADD COLUMN video_id uuid,
    ADD COLUMN review_id uuid,
    ALTER COLUMN body DROP NOT NULL,
    DROP CONSTRAINT messages_sender_role_check,
    ADD CONSTRAINT messages_sender_role_check
        CHECK (sender_role IN ('student', 'coach', 'system')),
    -- 種類ごとの中身: 本文は text だけ、動画は添削依頼と返却、添削は返却だけが持つ
    ADD CONSTRAINT messages_kind_shape CHECK (
        (kind = 'text' AND body IS NOT NULL AND video_id IS NULL AND review_id IS NULL)
        OR (kind = 'video_review_request' AND body IS NULL AND video_id IS NOT NULL AND review_id IS NULL)
        OR (kind = 'video_review_return' AND body IS NULL AND video_id IS NOT NULL AND review_id IS NOT NULL)
    ),
    -- 添削依頼の送り手は生徒、添削の返却の送り手はコーチ
    ADD CONSTRAINT messages_kind_sender CHECK (
        kind = 'text'
        OR (kind = 'video_review_request' AND sender_role = 'student')
        OR (kind = 'video_review_return' AND sender_role = 'coach')
    );

-- 同じ動画の添削依頼・添削の返却はスレッドに 1 通ずつ（同じ境界間イベントが 2 回届いても 1 通）
CREATE UNIQUE INDEX messages_one_per_video_and_kind
    ON messaging.messages (thread_id, kind, video_id)
    WHERE video_id IS NOT NULL;
