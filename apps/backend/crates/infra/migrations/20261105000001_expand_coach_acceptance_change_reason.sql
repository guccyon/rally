-- scheduling: 本部がコーチの受付を止めたときに書いた理由（D197。domain の CoachSuspensionReason、CoachAcceptanceChange の reason）。
-- 止めた記録（status = 'suspended'）は書いた理由を持つ（書かなければ空文字列。上限はレッスンメニューの非公開の理由
-- catalog.lesson_menu_publication_changes.reason と同じ 200 文字）。再開した記録（status = 'accepting'）は NULL。
-- 既存の行（この列を足す前の停止・再開）は NULL として読む（そのときの理由は残っていないので写さない）。
-- 「止めた記録は理由を持つ」は CHECK にしない（既存の止めた記録と、古い版のアプリが書く止めた記録は NULL になるため。
-- docs/adr/0004 の expand は古い版を壊さない）。「再開した記録は理由を持たない」は古い版も満たすので CHECK にする。
-- 理由の上限 200 文字の判定は domain の CoachSuspensionReason で行い、DB は壊れた値を入れないためだけに持つ。
-- 列を足すだけで、既存の列と制約は変えない（古い版のアプリは新しい列を読まず、書くときは NULL のまま）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE scheduling.coach_acceptance_changes
    ADD COLUMN reason text
        CONSTRAINT coach_acceptance_changes_reason_check CHECK (char_length(reason) <= 200),
    ADD CONSTRAINT coach_acceptance_changes_reason_shape
        CHECK (status = 'suspended' OR reason IS NULL);
