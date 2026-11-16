-- live_lesson: 通話セッションの参加者ごとの最初の入室時刻（Issue #1654。domain の LiveLessonSession::join と SessionJoin）。
-- 参加者（セッションのコーチと、結び付いた予約の生徒）が startLiveLessonSession で通話に入ったとき、その参加者の最初の
-- 入室を 1 行記録する。開始した参加者は開始の時刻に入室したものとして、セッションと同じトランザクションで書く。
-- セッションと参加者の組に高々 1 行（主キー）で、入り直しても行を書き換えない（最初の時刻を保つ）。
--
-- live_lesson.sessions の列と制約は変えない。この表の行がないセッション（この版より前に始めたもの）は、入室を記録する前の
-- 版として読む（コーチ・生徒の入室時刻は不明で null）。古い版のアプリはこの表を読み書きしないので、そのまま動く。
-- 参加者がセッションのコーチか結び付いた予約の生徒であることは domain で判定し、ここでは守らない（表をまたぐ）。
-- 表を足すだけで、既存の表は変えない（ADR 0004 の expand）。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

CREATE TABLE live_lesson.session_participants (
    session_id uuid        NOT NULL REFERENCES live_lesson.sessions (session_id),
    account_id uuid        NOT NULL REFERENCES identity.accounts (account_id),
    -- 最初に入室した時刻
    joined_at  timestamptz NOT NULL,
    CONSTRAINT session_participants_pkey PRIMARY KEY (session_id, account_id)
);
