-- scheduling: 予約した時点のレッスンメニューの写し（D64）。予約したスロットに結びついていた catalog.lesson_menus の
-- ID・名前・1 回の時間を予約の作成のときに写し、後でメニューが直されても予約の表示は予約した時点のままにする。
-- 予約 1 件に 0 か 1 つなので scheduling.bookings に NULL 可の列を足すだけ。既存の予約と、メニューなしの予約は NULL。
-- 写すかどうか（スロットに結びついたメニューで、予約の時点で公開中か）の判定は domain の Booking::with_lesson_menu の
-- 1 か所で行い、DB は壊れた値を入れないために写しの形だけを制約にする。メニューは catalog の持ち物なので外部キーは張らない
-- （外部キーは同じコンテキストの中と identity への参照だけ。ADR 0004 の 3）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE scheduling.bookings
    ADD COLUMN lesson_menu_id uuid,
    -- 名前は 1〜40 文字（全角空白などを含む空白だけの判定は domain の LessonMenuName。catalog.lesson_menus.name と同じ形）
    ADD COLUMN lesson_menu_name text
        CONSTRAINT bookings_lesson_menu_name_check
            CHECK (btrim(lesson_menu_name) <> '' AND char_length(lesson_menu_name) <= 40),
    -- 同期レッスン 1 回の時間（分。domain の LessonDuration）。予約した時点でメニューが非同期だったなら NULL
    ADD COLUMN lesson_menu_duration_minutes smallint
        CONSTRAINT bookings_lesson_menu_duration_check
            CHECK (lesson_menu_duration_minutes BETWEEN 15 AND 120 AND lesson_menu_duration_minutes % 15 = 0),
    -- ID と名前は揃って NULL か揃って値を持ち、時間を持てるのは写しのある予約だけ
    ADD CONSTRAINT bookings_lesson_menu_shape CHECK (
        (lesson_menu_id IS NULL) = (lesson_menu_name IS NULL)
        AND (lesson_menu_id IS NOT NULL OR lesson_menu_duration_minutes IS NULL)
    );
