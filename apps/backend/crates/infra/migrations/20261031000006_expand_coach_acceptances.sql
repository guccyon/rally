-- scheduling: コーチの受付（D116。domain の CoachAcceptance）。コーチが新しい予約を受け付けるか（受付中・停止中）を、
-- 本部が止める・再開する。コーチ 1 人に 0 か 1 行で、一度も止めていないコーチ（行がない）は受付中として扱う
-- （既存のコーチの行は足さない）。停止中のコーチの枠は生徒の検索（scheduling.slots の検索）に出さず、新しい予約を受けない。
-- 入っている予約と枠は変えない。止める・再開する規則は domain の CoachAcceptance の 1 か所で、ここでは値だけを守る。
-- 状態の値の対応は infra の scheduling_values。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE scheduling.coach_acceptances (
    coach_id   uuid        PRIMARY KEY REFERENCES identity.coaches (coach_id),
    status     text        NOT NULL CHECK (status IN ('accepting', 'suspended')),
    -- 最後に止めた・再開した時刻
    updated_at timestamptz NOT NULL DEFAULT now()
);
