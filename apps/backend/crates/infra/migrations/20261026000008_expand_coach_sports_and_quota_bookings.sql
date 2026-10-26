-- T18（合成シード）で identity / scheduling / entitlement / live-lesson / async-lesson の Postgres の保存先を足すときに、
-- domain のモデルを保存し切れなかった 2 つを足す。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

-- identity: コーチの提供スポーツ（domain の Coach.sports。1 つ以上）。accounts.sport は生徒だけが持つので、コーチは別の表に持つ。
-- 「1 つ以上」は domain の Coach::new で判定する（行の数は DB の制約にしない）
CREATE TABLE identity.coach_sports (
    coach_id uuid NOT NULL REFERENCES identity.coaches (coach_id),
    sport    text NOT NULL CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball')),
    PRIMARY KEY (coach_id, sport)
);

-- entitlement: ビデオ通話の暦月の枠を使った予約（ADR 0005 の決定 1。枠は「その月に枠を使った予約の集合」で持つ）。
-- live_lesson_quotas.remaining_live_lessons は上限 2 からこの行の数を引いた値と一緒に書く（3 回目は 0〜2 の CHECK で入らない）。
-- 予約は scheduling の持ち物なので booking_id に外部キーは張らない（ADR 0004 の 3）
CREATE TABLE entitlement.live_lesson_quota_bookings (
    entitlement_id uuid NOT NULL,
    month          date NOT NULL,
    booking_id     uuid NOT NULL,
    PRIMARY KEY (entitlement_id, month, booking_id),
    CONSTRAINT live_lesson_quota_bookings_quota_fkey FOREIGN KEY (entitlement_id, month)
        REFERENCES entitlement.live_lesson_quotas (entitlement_id, month)
);
