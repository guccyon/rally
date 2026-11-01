-- notification: 本部によるお知らせの取り下げ（D141 / Issue #473。domain の AnnouncementWithdrawal）。誤って送ったお知らせを
-- 本部が取り下げたことを、送った記録（notification.announcements）の行に、取り下げた本部のアカウントと時刻で残す。
-- 取り下げていなければどちらも NULL で、取り下げたらどちらも入る（片方だけは不可）。取り下げを戻す操作はない。
-- 生徒ごとのお知らせ（notification.notifications）の行は消さず、生徒の一覧で取り下げた記録を指すものを外す（既読も変えない）。
-- 二度目の取り下げを断る規則は domain の Announcement の 1 か所で、ここでは値の形だけを守る。
-- 列を足すだけで（どちらも NULL を許す）、既存の行と古い版のアプリの読み書きはそのまま通る（ADR 0004 の expand）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE notification.announcements
    -- 取り下げた本部のアカウント（役割は境界が確かめる。sender_id と同じ）
    ADD COLUMN withdrawn_by uuid        REFERENCES identity.accounts (account_id),
    ADD COLUMN withdrawn_at timestamptz,
    ADD CONSTRAINT announcements_withdrawal_check
        CHECK ((withdrawn_by IS NULL) = (withdrawn_at IS NULL));
