-- catalog: 本部によるレッスンメニューの非公開（D137。glossary の lesson_menu_headquarters_unpublish、domain の
-- HeadquartersUnpublish）。本部がコーチに代わってメニューを非公開にした時刻と、そのとき書いた理由（任意。書かなければ空文字列）。
-- メニュー 1 件に高々 1 つ（本部が公開に戻すと消える。履歴は持たない）なので catalog.lesson_menus に列を足す。
-- 既存のメニューと本部が非公開にしていないメニューは両方 NULL として読む。時刻と理由は揃って NULL か揃って値を持つ。
-- 本部が非公開にしたメニューをコーチが公開し直せないこと、理由の上限 200 文字の判定は domain の LessonMenu /
-- HeadquartersUnpublishReason で行い、DB は壊れた値を入れないためだけに持つ。
-- 「本部が非公開にしたなら is_published は false」は CHECK にしない（古い版のアプリは新しい列を知らずにコーチの変更で
-- is_published を書くので、CHECK にすると古い版の変更が失敗する。docs/adr/0004 の expand は古い版を壊さない）。
-- 列を足すだけで、既存の列と制約は変えない（古い版のアプリは新しい列を読まず、書くときは NULL のまま）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

ALTER TABLE catalog.lesson_menus
    ADD COLUMN headquarters_unpublished_at timestamptz,
    ADD COLUMN headquarters_unpublish_reason text
        CONSTRAINT lesson_menus_headquarters_unpublish_reason_check
            CHECK (char_length(headquarters_unpublish_reason) <= 200),
    ADD CONSTRAINT lesson_menus_headquarters_unpublish_shape
        CHECK ((headquarters_unpublished_at IS NULL) = (headquarters_unpublish_reason IS NULL));
