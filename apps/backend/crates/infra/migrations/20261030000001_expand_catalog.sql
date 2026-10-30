-- catalog: カタログ（specs/context-map.yaml の catalog）。コーチのレッスンメニュー（glossary の lesson「レッスン種別」。
-- スポーツごとにコーチが提供するメニューで、同期と非同期の別を持つ）。コーチとレッスン内容のセットが商品（docs/adr/0009）。
-- 新しいスキーマと表を足すだけで、既存の表は変えない。
-- 判定（コーチの提供スポーツか、メニューのコーチか、名前・説明の長さ、1 回の時間）は domain の LessonMenu と値の型で行い、
-- DB は壊れた値を入れないために同じ形を制約にする。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA catalog;

CREATE TABLE catalog.lesson_menus (
    lesson_menu_id   uuid        PRIMARY KEY,
    coach_id         uuid        NOT NULL,
    -- 値の並びは domain の Sport::ALL の順（tests/schema.rs が確かめる）
    sport            text        NOT NULL
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming')),
    -- infra の catalog_values（student-app の契約の LessonType と同じ値）
    lesson_type      text        NOT NULL CHECK (lesson_type IN ('live_lesson', 'async_lesson')),
    -- 名前は 1〜40 文字（全角空白などを含む空白だけの判定は domain の LessonMenuName）
    name             text        NOT NULL CHECK (btrim(name) <> '' AND char_length(name) <= 40),
    -- 説明は 0〜400 文字（domain の LessonMenuDescription）
    description      text        NOT NULL DEFAULT '' CHECK (char_length(description) <= 400),
    -- 同期レッスン 1 回の時間（分）。非同期は NULL
    duration_minutes smallint,
    is_published     boolean     NOT NULL,
    created_at       timestamptz NOT NULL,
    -- 同期は 15〜120 分の 15 分刻みの時間を持ち、非同期は持たない（domain の LessonFormat / LessonDuration）。
    -- CHECK は NULL を通すので、同期の時間が NULL でないことを明示する
    CONSTRAINT lesson_menus_duration_shape CHECK (
        (lesson_type = 'live_lesson' AND duration_minutes IS NOT NULL
         AND duration_minutes BETWEEN 15 AND 120 AND duration_minutes % 15 = 0)
        OR (lesson_type = 'async_lesson' AND duration_minutes IS NULL)
    ),
    -- スポーツはそのコーチの提供スポーツのどれか（コーチの表への参照も兼ねる）
    CONSTRAINT lesson_menus_coach_sport_fkey FOREIGN KEY (coach_id, sport)
        REFERENCES identity.coach_sports (coach_id, sport)
);

-- コーチごとのメニューの一覧（admin-web の listMyLessonMenus・student-app の listCoachLessonMenus）はコーチで読む
CREATE INDEX lesson_menus_coach_id_idx ON catalog.lesson_menus (coach_id, created_at);
