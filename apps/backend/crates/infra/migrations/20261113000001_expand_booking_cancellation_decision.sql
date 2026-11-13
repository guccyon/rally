-- scheduling: 予約のキャンセルの理由と判定（Issue #1031）。本部が予約 1 件の経緯（getBooking）を追えるように、キャンセルの
-- 理由と、そのキャンセルでビデオ通話の暦月の枠を使ったことになったかの判定を予約に残す。
--
-- 1. キャンセルした理由は、初版（20261026000002）からある scheduling.bookings.cancel_reason（text、500 文字まで、キャンセル
--    済みの予約だけ: bookings_cancel_reason_only_when_cancelled）に書く。これまで書くアプリはなく、どの行も NULL。新しい版は
--    domain の BookingCancelReason の値（infra の scheduling_values。今はコーチのお休み coach_leave だけ）を書き、生徒の
--    キャンセルは理由を残さない（NULL）。値を CHECK で絞るのは許す値を減らす変更（ADR 0004 の 2 の contract）なので、ここでは
--    しない。どのお休みでキャンセルしたかは #455 から残している scheduling.coach_leave_bookings（予約 1 件に高々 1 行）で
--    分かるので、お休みの記録への参照の列は足さない。
-- 2. scheduling.bookings に、キャンセル規定の判定 consumes_quota（domain の CancellationDecision。境界間イベント
--    BookingCancelled の consumes_quota と同じ値）を足す。判定は scheduling の CancellationPolicy の 1 か所で、ここには
--    その結果の記録だけを残す（DB で判定はしない。回数の消費の実行は entitlement）。キャンセル済みの予約だけが持つ。
--
-- NULL はこの版より前のキャンセルと、古い版のアプリが書いたキャンセル（理由・判定の記録なし）。新しい版のアプリは、
-- cancel_reason が NULL でも coach_leave_bookings に行のある予約はお休みでのキャンセルとして読む（domain の
-- Booking::with_restored_coach_leave）。列を足すだけで、既存の行と古い版のアプリの読み書きはそのまま通る。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE scheduling.bookings
    ADD COLUMN consumes_quota boolean;

ALTER TABLE scheduling.bookings
    ADD CONSTRAINT bookings_consumes_quota_only_when_cancelled
        CHECK (consumes_quota IS NULL OR status = 'cancelled');
