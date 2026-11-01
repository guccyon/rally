-- scheduling: コーチの受付を止めた・再開した記録（D147。domain の CoachAcceptanceChange）。本部が、どのコーチの受付を、
-- いつ止めて・いつ再開したか、誰（本部のアカウント）が操作したかを後からたどるための追記だけの表。止める・再開する 1 回に
-- 1 行で、scheduling.coach_acceptances の行と同じトランザクションで書く。書いた行は書き換えない。
-- status は変えた後の状態（suspended なら止めた、accepting なら再開した）で、値は scheduling.coach_acceptances.status と
-- 同じ（対応は infra の scheduling_values）。操作したのは role = 'headquarters' のアカウントだけ（役割つきの外部キー）。
-- この表を足す前の停止・再開の記録はない（scheduling.coach_acceptances は今の状態と最後に変えた時刻しか持たない）ので、
-- 写さない。止める・再開する規則は domain の CoachAcceptance の 1 か所で、ここでは値だけを守る。
-- 表を足すだけで、既存の表は変えない。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE scheduling.coach_acceptance_changes (
    -- 保存した順（同じ時刻の記録の並べ方に使う）
    seq             bigint      GENERATED ALWAYS AS IDENTITY,
    coach_id        uuid        NOT NULL REFERENCES identity.coaches (coach_id),
    status          text        NOT NULL CHECK (status IN ('accepting', 'suspended')),
    changed_by      uuid        NOT NULL,
    -- 止める・再開するのは role = 'headquarters' のアカウントだけ
    changed_by_role text        NOT NULL DEFAULT 'headquarters' CHECK (changed_by_role = 'headquarters'),
    changed_at      timestamptz NOT NULL,
    CONSTRAINT coach_acceptance_changes_pkey PRIMARY KEY (seq),
    CONSTRAINT coach_acceptance_changes_changed_by_fkey FOREIGN KEY (changed_by, changed_by_role)
        REFERENCES identity.accounts (account_id, role)
);

-- コーチ 1 人の記録を新しい順に読む（listCoachAcceptanceChanges）
CREATE INDEX coach_acceptance_changes_coach_id_changed_at_idx
    ON scheduling.coach_acceptance_changes (coach_id, changed_at DESC, seq DESC);
