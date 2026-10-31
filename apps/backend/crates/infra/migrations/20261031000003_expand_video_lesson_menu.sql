-- async_lesson: 動画を送った時点のレッスンメニューの写し（D67）。生徒が添削を頼むときに選んだ catalog.lesson_menus の
-- 非同期レッスンのメニューの ID・名前を動画の送付のときに写し、後でメニューが直されても動画の表示は送った時点のままにする。
-- 動画 1 本に 0 か 1 つなので async_lesson.videos に NULL 可の列を足すだけ。既存の動画と、メニューを選ばずに送った動画は NULL。
-- 選べるか（送り先のコーチの・公開中の・非同期の・動画と同じスポーツのメニューか）の判定は domain の Video::with_lesson_menu の
-- 1 か所で行い、DB は壊れた値を入れないために写しの形だけを制約にする。メニューは catalog の持ち物なので外部キーは張らない
-- （外部キーは同じコンテキストの中と identity への参照だけ。ADR 0004 の 3）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE async_lesson.videos
    ADD COLUMN lesson_menu_id uuid,
    -- 名前は 1〜40 文字（全角空白などを含む空白だけの判定は domain の LessonMenuName。catalog.lesson_menus.name と同じ形）
    ADD COLUMN lesson_menu_name text
        CONSTRAINT videos_lesson_menu_name_check
            CHECK (btrim(lesson_menu_name) <> '' AND char_length(lesson_menu_name) <= 40),
    -- ID と名前は揃って NULL か揃って値を持つ
    ADD CONSTRAINT videos_lesson_menu_shape
        CHECK ((lesson_menu_id IS NULL) = (lesson_menu_name IS NULL));
