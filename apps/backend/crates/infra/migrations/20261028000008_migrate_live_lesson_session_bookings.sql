-- live_lesson: この版より前に始めた通話セッションの、開始した予約を session_bookings に写す（D41。1 つ前の expand で足した表）。
-- 写した後も、行のないセッション（古い版のアプリが写した後に始めたもの）は保存先の実装が開始した予約だけのものとして読む。
-- 変更の規約は docs/adr/0004-schema-expand-contract.md（このファイルは適用後に書き換えない）。

INSERT INTO live_lesson.session_bookings (session_id, booking_id, student_id, position)
SELECT session_id, booking_id, student_id, 0
FROM live_lesson.sessions
ON CONFLICT DO NOTHING;
