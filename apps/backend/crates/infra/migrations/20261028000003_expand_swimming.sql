-- 提供スポーツに水泳（swimming）を足す（変更シナリオ cs-001。specs/change-scenarios.yaml）。
-- 水泳は月額（ゴルフ以外）なので、月額の契約の表（ゴルフを含まない制約）にも足す。
-- sport の列を持つすべての表の CHECK 制約を、同じ名前で値を 1 つ足して付け直す（許す値を広げるだけの緩和。
-- 既存の値はそのまま通り、古い版のアプリもそのまま動く）。値の並びは domain の Sport::ALL の順（tests/schema.rs が確かめる）。
-- 付け直しは制約の DROP と ADD を 1 つの ALTER TABLE に書く（間に制約のない瞬間を作らない）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

-- identity: 生徒のスポーツ
ALTER TABLE identity.accounts
    DROP CONSTRAINT accounts_sport_check,
    ADD CONSTRAINT accounts_sport_check
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming'));

-- identity: コーチの提供スポーツ
ALTER TABLE identity.coach_sports
    DROP CONSTRAINT coach_sports_sport_check,
    ADD CONSTRAINT coach_sports_sport_check
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming'));

-- scheduling: 予約枠
ALTER TABLE scheduling.slots
    DROP CONSTRAINT slots_sport_check,
    ADD CONSTRAINT slots_sport_check
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming'));

-- entitlement: 月額契約（ゴルフ以外）
ALTER TABLE entitlement.subscriptions
    DROP CONSTRAINT subscriptions_sport_check,
    ADD CONSTRAINT subscriptions_sport_check
        CHECK (sport IN ('tennis', 'baseball', 'soccer', 'basketball', 'swimming'));

-- entitlement: 受講権（月額 / ワンショットの形は entitlements_subscription_shape / one_shot_shape のまま。水泳は月額の形に入る）
ALTER TABLE entitlement.entitlements
    DROP CONSTRAINT entitlements_sport_check,
    ADD CONSTRAINT entitlements_sport_check
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming'));

-- async-lesson: 生徒が送った動画
ALTER TABLE async_lesson.videos
    DROP CONSTRAINT videos_sport_check,
    ADD CONSTRAINT videos_sport_check
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming'));
