-- matching: マッチング（specs/context-map.yaml の matching）。生徒のお気に入りのコーチ（D126。glossary の favorite_coach、
-- domain の FavoriteCoach）。生徒とコーチの組で 1 行（favorite_coaches_pkey）で、同じコーチを何度入れても 1 件。
-- お気に入りを持てるのは生徒だけで、入れる相手はコーチだけ（判定は domain の FavoriteCoach::new。DB は役割つきの
-- 外部キーで同じ形を守る）。matching が持つ最初の表なので、スキーマも足す。
-- 新しいスキーマと表を足すだけで、既存の表は変えない。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE SCHEMA matching;

CREATE TABLE matching.favorite_coaches (
    student_id   uuid        NOT NULL,
    -- favorite_coaches に入れられるのは role = 'student' のアカウントのお気に入りだけ
    student_role text        NOT NULL DEFAULT 'student' CHECK (student_role = 'student'),
    coach_id     uuid        NOT NULL REFERENCES identity.coaches (coach_id),
    -- お気に入りに入れた時刻（入れ直しても最初の時刻のまま）
    favorited_at timestamptz NOT NULL,
    -- 生徒のお気に入りの一覧（listFavoriteCoaches）は生徒で読むので、生徒を先頭にする
    CONSTRAINT favorite_coaches_pkey PRIMARY KEY (student_id, coach_id),
    CONSTRAINT favorite_coaches_student_fkey FOREIGN KEY (student_id, student_role)
        REFERENCES identity.accounts (account_id, role)
);
