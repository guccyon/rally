-- identity: 生徒のログインの確認コード（student-app の requestLoginCode・createSession。docs/adr/0012-student-login-one-time-code.md）。
-- メールアドレスごと（大文字小文字を区別しない。accounts_email_key と同じ lower(email)）に 1 行で、送り直すと置き換え、
-- 確かめて合ったら消す。生徒のアカウントのないメールアドレス（登録がない・コーチや本部のもの）にも、送らないコードの行を
-- account_id を NULL で作る（送り直しの待ち・期限・間違いの上限を登録の有無で変えず、応答から登録の有無を明かさないため）。
-- 期限（送ってから 10 分）・送り直しの待ち（60 秒）・間違いの上限（5 回）は domain の LoginCodeChallenge が決め、DB は持たない。
-- 表を足すだけで、既存の表は変えない（古い版のアプリはこの表を読まない）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE identity.login_codes (
    -- lower(email)。メールアドレスごとに 1 つ
    email_key       text        PRIMARY KEY,
    -- リクエストのままのメールアドレス
    email           text        NOT NULL,
    -- コードでログインする生徒。生徒のアカウントのないメールアドレスなら NULL（コードは送っていない）
    account_id      uuid,
    -- account_id に入れられるのは role = 'student' のアカウントだけ
    account_role    text        NOT NULL DEFAULT 'student' CHECK (account_role = 'student'),
    -- 6 桁の数字（domain の LoginCode）
    code            text        NOT NULL CHECK (code ~ '^[0-9]{6}$'),
    issued_at       timestamptz NOT NULL,
    failed_attempts integer     NOT NULL DEFAULT 0 CHECK (failed_attempts >= 0),
    CONSTRAINT login_codes_email_key_check CHECK (email_key = lower(email)),
    CONSTRAINT login_codes_account_fkey FOREIGN KEY (account_id, account_role)
        REFERENCES identity.accounts (account_id, role)
);
