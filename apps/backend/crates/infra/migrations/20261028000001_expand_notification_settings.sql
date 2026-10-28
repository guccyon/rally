-- notification: 通知（specs/context-map.yaml の notification）。通知の受け取り設定（アカウントごと、通知の種類ごとに
-- 受け取るか）。行のない種類は既定のオン（domain の NotificationSettings::new）として読むので、変えたことのない
-- アカウントには行がない。通知そのものは保存しない（送るだけ）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA notification;

CREATE TABLE notification.notification_settings (
    account_id uuid    NOT NULL REFERENCES identity.accounts (account_id),
    -- 通知の種類（domain の NotificationKind。値の対応は infra の notification_values）
    kind       text    NOT NULL CHECK (kind IN ('booking_confirmed', 'video_review_returned', 'message_received')),
    -- 受け取るか（オン / オフ）
    enabled    boolean NOT NULL,
    -- アカウントの同じ種類の設定は 1 つ
    CONSTRAINT notification_settings_pkey PRIMARY KEY (account_id, kind)
);
