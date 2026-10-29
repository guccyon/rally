-- scheduling: スロットの定員（domain の SlotCapacity。変更シナリオ cs-004「グループレッスン（1 コーチ : N 生徒）を追加する」の 1 本目）。
-- 枠 1 つに 1 つなので scheduling.slots に列を足す。既存の枠は定員 1（今までのマンツーマンと同じ）として読む。
-- 定員の範囲は domain の SlotCapacity::MIN / MAX と同じ 1〜6。判定は domain で行い、DB は壊れた値を入れないためだけに持つ。
-- 有効な予約の数（未確定・確定）は列に持たず、予約の行から数える（domain の SlotSeats）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE scheduling.slots
    ADD COLUMN capacity smallint NOT NULL DEFAULT 1
        CONSTRAINT slots_capacity_check CHECK (capacity BETWEEN 1 AND 6);

-- 同じ生徒が同じ枠に持てる有効な（キャンセルされていない）予約は 1 件まで（判定は domain の Booking::reserve。DB は壊れた値を
-- 入れないため）。下で外す 1 枠 1 予約の索引より緩い一意性で、定員 1 の枠では 1 枠 1 予約の索引が守っていたものを含む。
CREATE UNIQUE INDEX bookings_one_active_per_slot_student
    ON scheduling.bookings (slot_id, student_id) WHERE status <> 'cancelled';

-- 1 つの枠に有効な予約は 1 件まで、の一意の索引を外す（定員まで複数の予約を持てるようにする一意性の緩和。データは消えない）。
-- 定員を超える予約の防止は domain の SlotSeats と、予約の作成でスロットの行を FOR UPDATE でロックしてから数える PgBookingRepository が守る。
-- 古い版のアプリも同じロックの中で 1 枠 1 予約を判定するので、この索引がなくても二重予約は入らない（ADR 0004 の 2 の緩和）。
-- relax: 定員まで複数の予約を持てるようにする一意性の緩和（上の 1 生徒 1 予約の索引と、スロットの行のロックの中の数え上げが守る）
DROP INDEX scheduling.bookings_one_active_per_slot;
