-- messaging: メッセージの種類にスタンプを足す（D54 / Issue #227。docs/adr/0007 の決定 9）。
-- スタンプは当事者（生徒・コーチ）が本文の代わりに送るもので、決まった 8 種類の ID（stamp）のどれかだけを持ち、本文・動画・添削は持たない。
-- 種類（kind）に stamp を足す（許す値を広げるだけの付け直し）。既存の行の stamp は NULL。
-- 種類ごとの形（messages_kind_shape）と送り手（messages_kind_sender）は、stamp の場合を OR で足すだけの付け直し（既存の種類の条件は変えない）。
-- stamp の列を持つのが stamp の種類だけであることは、新しい制約 messages_stamp_only_for_stamp で守る（既存の行はすべて満たす）。
-- 種類ごとの形と送り手の判定は domain の MessageContent で、DB は壊れた値を入れないために守る。ID の値と domain の Stamp の対応は
-- infra の messaging_values（CHECK との一致は tests/schema.rs）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE messaging.messages
    ADD COLUMN stamp text
        CONSTRAINT messages_stamp_check
        CHECK (stamp IN ('thanks', 'good', 'got_it', 'lets_go', 'nice_shot', 'tired', 'question', 'ok')),
    DROP CONSTRAINT messages_kind_check,
    ADD CONSTRAINT messages_kind_check
        CHECK (kind IN ('text', 'video_review_request', 'video_review_return', 'stamp')),
    ADD CONSTRAINT messages_stamp_only_for_stamp CHECK (kind = 'stamp' OR stamp IS NULL);

-- relax: 種類ごとの中身に stamp（本文・動画・添削を持たずスタンプの ID だけ）を OR で足すだけ（既存の種類の条件は同じ。検査は値の増減を比べられない形）
ALTER TABLE messaging.messages
    DROP CONSTRAINT messages_kind_shape,
    ADD CONSTRAINT messages_kind_shape CHECK (
        (kind = 'text' AND body IS NOT NULL AND video_id IS NULL AND review_id IS NULL)
        OR (kind = 'video_review_request' AND body IS NULL AND video_id IS NOT NULL AND review_id IS NULL)
        OR (kind = 'video_review_return' AND body IS NULL AND video_id IS NOT NULL AND review_id IS NOT NULL)
        OR (kind = 'stamp' AND body IS NULL AND video_id IS NULL AND review_id IS NULL AND stamp IS NOT NULL)
    );

-- relax: スタンプの送り手は当事者（生徒・コーチ）を OR で足すだけ（既存の種類の条件は同じ。検査は値の増減を比べられない形）
ALTER TABLE messaging.messages
    DROP CONSTRAINT messages_kind_sender,
    ADD CONSTRAINT messages_kind_sender CHECK (
        kind = 'text'
        OR (kind = 'video_review_request' AND sender_role = 'student')
        OR (kind = 'video_review_return' AND sender_role = 'coach')
        OR (kind = 'stamp' AND sender_role IN ('student', 'coach'))
    );
