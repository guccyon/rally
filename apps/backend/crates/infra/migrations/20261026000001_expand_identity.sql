-- identity: アカウント・権限（specs/context-map.yaml）。
-- 生徒・コーチ・本部を accounts.role で区別し、コーチとしての情報は coaches に分ける。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA identity;

CREATE TABLE identity.accounts (
    account_id   uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    -- student: 生徒 / coach: コーチ / headquarters: 本部
    role         text        NOT NULL CHECK (role IN ('student', 'coach', 'headquarters')),
    email        text        NOT NULL,
    display_name text        NOT NULL CHECK (char_length(display_name) >= 1),
    -- 生徒のスポーツ（student-app の Student.sport）。生徒だけが持つ
    sport        text        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball')),
    created_at   timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT accounts_sport_only_for_students CHECK ((role = 'student') = (sport IS NOT NULL)),
    -- 他のテーブルが「この役割のアカウント」を (account_id, role) の複合外部キーで参照するため
    CONSTRAINT accounts_account_id_role_key UNIQUE (account_id, role)
);

CREATE UNIQUE INDEX accounts_email_key ON identity.accounts (lower(email));

CREATE TABLE identity.coaches (
    coach_id     uuid        PRIMARY KEY,
    -- coaches に入れられるのは role = 'coach' のアカウントだけ
    account_role text        NOT NULL DEFAULT 'coach' CHECK (account_role = 'coach'),
    created_at   timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT coaches_account_fkey FOREIGN KEY (coach_id, account_role)
        REFERENCES identity.accounts (account_id, role)
);
