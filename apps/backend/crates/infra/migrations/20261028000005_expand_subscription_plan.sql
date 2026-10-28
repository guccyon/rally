-- entitlement: 月額契約のプラン（domain の SubscriptionPlan。変更シナリオ cs-003「ビデオ通話の月間回数を契約プランごとに可変にする」）。
-- プランはビデオ通話の暦月の上限を決める（standard 2 回・plus 4 回。数は domain の SubscriptionPlan::live_lessons_per_month の 1 か所だけ）。
-- 契約 1 つに 1 つなので entitlement.subscriptions に列を足す。既存の契約は standard（今までの暦月 2 回と同じ）として読む。
-- 料金と決済は billing の持ち物で、ここには持たない（Gen1 の範囲外）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE entitlement.subscriptions
    ADD COLUMN plan text NOT NULL DEFAULT 'standard'
        CONSTRAINT subscriptions_plan_check CHECK (plan IN ('standard', 'plus'));

-- 暦月の枠の残回数は、上限がプランで変わる（plus は 4 から数える）ので、上の端を外して 0 以上だけを守る（許す値を広げるだけの
-- 緩和。同じ名前で付け直す）。使い切った後の消費は今までどおり負になって入らない。プランごとの上限の判定は domain の
-- LiveLessonQuota が行い、DB には上限の数を書かない。既定値の 2（standard の上限）も外す（アプリは残回数を必ず書く）。
ALTER TABLE entitlement.live_lesson_quotas
    DROP CONSTRAINT live_lesson_quotas_remaining_live_lessons_check,
    ADD CONSTRAINT live_lesson_quotas_remaining_live_lessons_check
        CHECK (remaining_live_lessons >= 0),
    ALTER COLUMN remaining_live_lessons DROP DEFAULT;
