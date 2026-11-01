-- notification: 本部からのお知らせの下書き（D177 / Issue #555。domain の AnnouncementDraft）。本部が書きかけで残した
-- お知らせを、送った記録（notification.announcements）とは別の表に 1 件 1 行で残す。下書きは生徒に届かないので、
-- 生徒ごとのお知らせ（notification.notifications）は作らない。下書きから送ったら、送った記録を足して下書きの行を消す
-- （行の削除はアプリの操作で、スキーマの変更ではない）。
-- 書きかけなので題・本文は空でもよく、上限（題 50 文字・本文 2000 文字）だけを守る。送るときの空の判定は domain の
-- AnnouncementDraft::sendable（送るお知らせと同じ規則）の 1 か所で、ここでは値の形だけを守る。
-- 表を足すだけで、既存の行と古い版のアプリの読み書きはそのまま通る（ADR 0004 の expand）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE notification.announcement_drafts (
    announcement_draft_id uuid        PRIMARY KEY,
    -- 作った順（同じ時刻に直した下書きを後に作ったものから並べる）
    seq                   bigint      GENERATED ALWAYS AS IDENTITY,
    -- 作った本部のアカウント（役割は境界が確かめる。announcements.sender_id と同じ）
    created_by            uuid        NOT NULL REFERENCES identity.accounts (account_id),
    -- 題（0〜50 文字）と本文（0〜2000 文字）。上限は domain の AnnouncementTitle / AnnouncementBody と契約の maxLength
    title                 text        NOT NULL CHECK (char_length(title) <= 50),
    body                  text        NOT NULL CHECK (char_length(body) <= 2000),
    -- 対象のスポーツ（identity.accounts.sport と同じ値）。生徒全員に送る下書きなら NULL
    sport                 text
        CHECK (sport IN ('tennis', 'golf', 'baseball', 'soccer', 'basketball', 'swimming')),
    created_at            timestamptz NOT NULL,
    -- 最後に直した時刻（直していなければ created_at）
    updated_at            timestamptz NOT NULL
);

-- 下書きを最後に直した時刻の新しい順に読む
CREATE INDEX announcement_drafts_updated_at
    ON notification.announcement_drafts (updated_at DESC, seq DESC);
