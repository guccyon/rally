-- scheduling: スロットの状態（T07）。open: 公開中で未予約 / booked: 予約で埋まっている / closed: コーチが閉じた。
-- 同じコーチの枠が重ならないことは domain の CoachCalendar で判定し、DB の排他制約にはしない（ADR 0004 の 4）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE scheduling.slots
    ADD COLUMN status text NOT NULL DEFAULT 'open'
        CONSTRAINT slots_status_check CHECK (status IN ('open', 'booked', 'closed'));

-- 生徒の検索（公開中の枠をスポーツで絞り、開始時刻・ID の順にページングする）
CREATE INDEX slots_open_sport_starts_at_idx ON scheduling.slots (sport, starts_at, slot_id) WHERE status = 'open';
