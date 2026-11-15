-- async_lesson: 生徒が動画を送るときに書いたコーチへの相談内容（student-app の submitVideo の note。#1635）。
-- これまでは AI の一次フィードバックの材料に渡すだけで保存しておらず、コーチに届かなかった。動画とともに保存し、送り先の
-- コーチが admin-web の listMyCoachVideos で読めるようにする。送った時点のまま変わらない。
-- 動画 1 本に 0 か 1 つなので async_lesson.videos に NULL 可の列を足すだけ。既存の動画と、書かずに送った動画は NULL
-- （埋め戻さない）。空白だけなら書かなかったのと同じ（NULL）にする判定は domain の VideoNote の 1 か所で行い
-- （全角空白などを含むので DB では守らない）、DB は壊れた値を入れないために上限だけを制約にする。
-- 列を足すだけで、既存の列と制約は変えない（古い版のアプリは新しい列を読まず、動画を書くときは NULL のまま）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE async_lesson.videos
    -- 1000 文字まで（domain の VideoNote::MAX_CHARS。student-app の契約の maxLength）
    ADD COLUMN note text
        CONSTRAINT videos_note_check CHECK (char_length(note) <= 1000);
