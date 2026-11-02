-- catalog: 本部がレッスンメニューを非公開にした・公開に戻した記録（D187。glossary の lesson_menu_publication_change、domain の
-- LessonMenuPublicationChange）。本部が、どのメニューを、いつ非公開にして・いつ公開に戻したか、誰（本部のアカウント）が
-- 操作したかを後からたどるための追記だけの表。非公開にする・公開に戻す 1 回に 1 行で、catalog.lesson_menus の行と同じ
-- トランザクションで書く。書いた行は書き換えない。
-- action は操作（unpublished なら非公開にした、republished なら公開に戻した。対応は infra の catalog_values）。reason は
-- 非公開にしたときに書いた理由（書かなければ空文字列。上限は catalog.lesson_menus.headquarters_unpublish_reason と同じ 200 文字）で、
-- 非公開にした記録だけが持ち、公開に戻した記録は NULL。操作したのは role = 'headquarters' のアカウントだけ（役割つきの外部キー）。
-- この表を足す前の非公開・公開に戻す操作の記録はない（catalog.lesson_menus は今の非公開の時刻と理由しか持たず、誰が操作したかが
-- 残っていない）ので、写さない。コーチが自分で公開・非公開を変えたことは記録しない。非公開にする・公開に戻す規則は domain の
-- LessonMenu の 1 か所で、ここでは値だけを守る。
-- 表を足すだけで、既存の表は変えない（古い版のアプリはこの表を知らず、非公開・公開に戻しても記録を書かない）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE catalog.lesson_menu_publication_changes (
    -- 保存した順（同じ時刻の記録の並べ方に使う）
    seq             bigint      GENERATED ALWAYS AS IDENTITY,
    lesson_menu_id  uuid        NOT NULL REFERENCES catalog.lesson_menus (lesson_menu_id),
    action          text        NOT NULL CHECK (action IN ('unpublished', 'republished')),
    reason          text
        CONSTRAINT lesson_menu_publication_changes_reason_check CHECK (char_length(reason) <= 200),
    changed_by      uuid        NOT NULL,
    -- 非公開にする・公開に戻すのは role = 'headquarters' のアカウントだけ
    changed_by_role text        NOT NULL DEFAULT 'headquarters' CHECK (changed_by_role = 'headquarters'),
    changed_at      timestamptz NOT NULL,
    CONSTRAINT lesson_menu_publication_changes_pkey PRIMARY KEY (seq),
    -- 理由を持つのは非公開にした記録だけ
    CONSTRAINT lesson_menu_publication_changes_reason_shape
        CHECK ((action = 'unpublished') = (reason IS NOT NULL)),
    CONSTRAINT lesson_menu_publication_changes_changed_by_fkey FOREIGN KEY (changed_by, changed_by_role)
        REFERENCES identity.accounts (account_id, role)
);

-- メニュー 1 件の記録を新しい順に読む（listLessonMenuPublicationChanges）
CREATE INDEX lesson_menu_publication_changes_lesson_menu_id_changed_at_idx
    ON catalog.lesson_menu_publication_changes (lesson_menu_id, changed_at DESC, seq DESC);
