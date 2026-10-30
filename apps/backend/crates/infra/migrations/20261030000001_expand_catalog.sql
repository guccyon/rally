-- catalog: カタログ（specs/context-map.yaml の catalog）。コーチがスポーツごとに提供するレッスンのメニュー
-- （glossary の lesson。レッスン種別）。同期レッスン（live_lesson）と非同期レッスン（async_lesson）の別を持ち、
-- 同期レッスンだけが 1 回の時間（分）を持つ。メニューは消さず、非公開（is_published = false）にする。
-- 既存の表は変えない（新しいスキーマと表を足すだけの expand）。
-- スポーツがそのコーチの提供スポーツ（identity.coach_sports）かは domain の LessonMenu::create で判定する
-- （外部キーは identity の accounts / coaches への参照だけに限る。ADR 0004 の 3）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA catalog;

CREATE TABLE catalog.lesson_menus (
    lesson_menu_id   uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    coach_id         uuid        NOT NULL REFERENCES identity.coaches (coach_id),
    -- 値の並びは domain の Sport::ALL の順（tests/schema.rs が確かめる）
    sport            text        NOT NULL
        CONSTRAINT lesson_menus_sport_check
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming')),
    -- 値の並びは domain の LessonType::ALL の順（infra の catalog_values。tests/schema.rs が確かめる）
    lesson_type      text        NOT NULL
        CONSTRAINT lesson_menus_lesson_type_check
        CHECK (lesson_type IN ('live_lesson', 'async_lesson')),
    -- 名前。空と半角空白だけは入らず、40 文字まで（全角空白などを含む空白の判定は domain の LessonMenuName）
    name             text        NOT NULL
        CONSTRAINT lesson_menus_name_check
        CHECK (btrim(name) <> '' AND char_length(name) <= 40),
    -- 説明。空でもよく、400 文字まで（domain の LessonMenuDescription）
    description      text        NOT NULL DEFAULT ''
        CONSTRAINT lesson_menus_description_check
        CHECK (char_length(description) <= 400),
    -- 同期レッスンの 1 回の時間（分）。15〜120 の 15 分刻み（domain の LessonDuration）
    duration_minutes smallint
        CONSTRAINT lesson_menus_duration_minutes_check
        CHECK (duration_minutes BETWEEN 15 AND 120 AND duration_minutes % 15 = 0),
    is_published     boolean     NOT NULL,
    created_at       timestamptz NOT NULL,
    updated_at       timestamptz NOT NULL,
    -- 同期レッスンだけが 1 回の時間を持つ
    CONSTRAINT lesson_menus_duration_shape
        CHECK ((lesson_type = 'live_lesson') = (duration_minutes IS NOT NULL)),
    CONSTRAINT lesson_menus_updated_after_created CHECK (updated_at >= created_at)
);

-- コーチの自分の一覧（admin-web の listMyLessonMenus）と生徒が見るコーチの一覧（student-app の listCoachLessonMenus）はコーチで読む
CREATE INDEX lesson_menus_coach_id_idx ON catalog.lesson_menus (coach_id);
