-- entitlement: 受講権（specs/context-map.yaml）。契約種別（月額 / ワンショット）と利用権（残回数・送り放題）。
-- 月額はゴルフ以外、ワンショットはゴルフだけ。Gen1 で実装するのは月額のみだが、型として両方を持つ。
-- 月額で消費されるのはビデオ通話の暦月 2 回の枠だけ（不変条件 1）。枠は暦月ごとに live_lesson_quotas に 1 行持つ。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA entitlement;

CREATE TABLE entitlement.subscriptions (
    subscription_id uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id      uuid        NOT NULL,
    student_role    text        NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    -- 月額はゴルフ以外
    sport           text        NOT NULL CHECK (sport IN ('tennis', 'baseball', 'soccer', 'basketball')),
    status          text        NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'cancelled')),
    started_at      timestamptz NOT NULL DEFAULT now(),
    cancelled_at    timestamptz,
    CONSTRAINT subscriptions_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role),
    CONSTRAINT subscriptions_status_cancelled_at CHECK ((status = 'cancelled') = (cancelled_at IS NOT NULL)),
    -- entitlements が「同じ生徒・同じスポーツの月額」を複合外部キーで参照するため
    CONSTRAINT subscriptions_id_student_sport_key UNIQUE (subscription_id, student_id, sport)
);

CREATE INDEX subscriptions_student_id_idx ON entitlement.subscriptions (student_id);

CREATE TABLE entitlement.entitlements (
    entitlement_id           uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id               uuid        NOT NULL,
    student_role             text        NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    contract_type            text        NOT NULL CHECK (contract_type IN ('subscription', 'one_shot')),
    sport                    text        NOT NULL CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball')),
    -- 月額の受講権の元になった契約（ワンショットは null）
    subscription_id          uuid,
    -- 動画とチャットが送り放題か（月額は true）
    unlimited_video_and_chat boolean     NOT NULL,
    -- 残りの動画添削回数（ワンショットのみ。月額は null）
    remaining_video_reviews  integer     CHECK (remaining_video_reviews >= 0),
    created_at               timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT entitlements_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role),
    CONSTRAINT entitlements_subscription_fkey FOREIGN KEY (subscription_id, student_id, sport)
        REFERENCES entitlement.subscriptions (subscription_id, student_id, sport),
    CONSTRAINT entitlements_subscription_id_key UNIQUE (subscription_id),
    CONSTRAINT entitlements_subscription_shape CHECK (
        contract_type <> 'subscription' OR (
            subscription_id IS NOT NULL AND sport <> 'golf'
            AND unlimited_video_and_chat AND remaining_video_reviews IS NULL
        )
    ),
    CONSTRAINT entitlements_one_shot_shape CHECK (
        contract_type <> 'one_shot' OR (
            subscription_id IS NULL AND sport = 'golf' AND remaining_video_reviews IS NOT NULL
        )
    ),
    -- live_lesson_quotas が「月額の受講権」を複合外部キーで参照するため
    CONSTRAINT entitlements_id_contract_type_key UNIQUE (entitlement_id, contract_type)
);

CREATE INDEX entitlements_student_id_idx ON entitlement.entitlements (student_id);

-- ビデオ通話の暦月の枠（月額のみ）。3 回目は残回数が負になるので入らない（不変条件 1）
CREATE TABLE entitlement.live_lesson_quotas (
    entitlement_id         uuid     NOT NULL,
    contract_type          text     NOT NULL DEFAULT 'subscription' CHECK (contract_type = 'subscription'),
    -- 暦月の初日（API の LiveLessonQuota.month の YYYY-MM に当たる）
    month                  date     NOT NULL CHECK (extract(day FROM month) = 1),
    remaining_live_lessons smallint NOT NULL DEFAULT 2 CHECK (remaining_live_lessons BETWEEN 0 AND 2),
    PRIMARY KEY (entitlement_id, month),
    CONSTRAINT live_lesson_quotas_entitlement_fkey FOREIGN KEY (entitlement_id, contract_type)
        REFERENCES entitlement.entitlements (entitlement_id, contract_type)
);
