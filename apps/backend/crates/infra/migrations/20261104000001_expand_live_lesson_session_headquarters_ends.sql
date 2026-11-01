-- live_lesson: 本部が終わったことにした通話セッション（D188。domain の LiveLessonSession::end_by_headquarters）。
-- 終了の記録がないまま残ったセッション（listOverdueLiveLessonSessions で見つけたもの）を、本部の判断で終わらせた記録。
-- セッション 1 つに高々 1 行（主キー）で、終わった時刻 ended_at（操作の時刻）と、終わらせた本部のアカウント ended_by を持つ。
-- 書いた行は書き換えない。終わらせられるのは role = 'headquarters' のアカウントだけ（役割つきの複合外部キー）。
--
-- live_lesson.sessions の列と制約は変えない。本部が終わらせたときは通話基盤から接続品質の指標を受け取らないので、
-- sessions.ended_at は NULL のまま残し（sessions_quality_when_ended は終了の時刻と指標を揃えて求める）、終わったことは
-- この表の行で表す。新しい版のアプリは「sessions.ended_at があるか、この表に行がある」を終了として読む。古い版のアプリは
-- この表を読まないので、本部が終わらせたセッションを今までどおり通話中として読む（読み書きは失敗しない）。
-- 終了が開始の後であること・終了済みのセッションを終わらせないことは domain で判定し、ここでは守らない
-- （開始の時刻は sessions にあり、表をまたぐ CHECK は書けない）。
-- 表を足すだけで、既存の表は変えない（ADR 0004 の expand）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE live_lesson.session_headquarters_ends (
    session_id    uuid        NOT NULL REFERENCES live_lesson.sessions (session_id),
    ended_by      uuid        NOT NULL,
    -- 終わらせるのは role = 'headquarters' のアカウントだけ
    ended_by_role text        NOT NULL DEFAULT 'headquarters' CHECK (ended_by_role = 'headquarters'),
    -- 終わった時刻（本部が操作した時刻）
    ended_at      timestamptz NOT NULL,
    CONSTRAINT session_headquarters_ends_pkey PRIMARY KEY (session_id),
    CONSTRAINT session_headquarters_ends_ended_by_fkey FOREIGN KEY (ended_by, ended_by_role)
        REFERENCES identity.accounts (account_id, role)
);
