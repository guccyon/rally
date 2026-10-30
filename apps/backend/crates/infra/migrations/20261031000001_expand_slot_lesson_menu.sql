-- scheduling: 予約枠に結びついたコーチのレッスンメニュー（catalog.lesson_menus。D61）。
-- 枠 1 つに 0 か 1 つなので scheduling.slots に NULL 可の列を足すだけ。既存の枠と、メニューを指定せずに公開した枠は NULL。
-- 結びつけられるメニュー（枠のコーチ自身の・公開中の・同期の・スポーツが枠と同じ・1 回の時間が枠の長さと同じ）の判定は
-- domain の Slot::with_lesson_menu の 1 か所で行う。メニューは catalog の持ち物なので外部キーは張らない
-- （外部キーは同じコンテキストの中と identity への参照だけ。ADR 0004 の 3）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE scheduling.slots
    ADD COLUMN lesson_menu_id uuid;
